use emux_daemon::client::DaemonClient;
use emux_ipc::{ClientMessage, ServerMessage};
use emux_mux::Domain;

use crate::AppError;
use crate::daemon::{list_live_sessions, socket_path_for, start_daemon_server};
use crate::event_loop::run_attached;

pub(crate) fn print_help() {
    println!("emux — a terminal multiplexer\n");
    println!("Usage: emux [command] [options]\n");
    println!("Commands:");
    println!("  new [name]       Start a new session (optional name)");
    println!("  attach [name]    Attach to an existing session");
    println!("  list, ls         List active sessions");
    println!("  kill <name>      Kill a session");
    println!("  ssh <dest> [cmd] Connect to a remote emux session via SSH");
    println!("                   dest: [user@]host[:port]");
    println!("                   cmd:  new [name] | attach [name] | list");
    println!("  upgrade          Update emux to the latest version\n");
    println!("Options:");
    println!("  -h, --help       Show this help message");
    println!("  -V, --version    Show version");
    println!("\nKeybindings (default leader: Ctrl+Shift):");
    println!("  Leader + d       Split pane down");
    println!("  Leader + r       Split pane right");
    println!("  Leader + x       Close pane");
    println!("  Leader + t       New tab");
    println!("  Leader + w       Close tab");
    println!("  Leader + n       Next tab");
    println!("  Leader + p       Previous tab");
    println!("  Leader + q       Detach session");
}

/// Generate a default session name like "0", "1", etc.
pub(crate) fn generate_session_name() -> String {
    let sessions = list_live_sessions();
    let mut i = 0u32;
    loop {
        let name = i.to_string();
        if !sessions.iter().any(|(n, _)| n == &name) {
            return name;
        }
        i += 1;
    }
}

/// `emux` (no args) — always create a new daemon session.
pub(crate) fn cmd_default() -> Result<(), AppError> {
    let name = generate_session_name();
    cmd_new(&name)
}

/// `emux new [name]` — force-create a new session and attach.
pub(crate) fn cmd_new(session_name: &str) -> Result<(), AppError> {
    // Start the daemon server in a background thread.
    start_daemon_server(session_name)?;
    // Attach to it.
    run_attached(session_name)
}

/// `emux attach [name]` — attach to an existing session.
pub(crate) fn cmd_attach(session_name: &str) -> Result<(), AppError> {
    let path = socket_path_for(session_name);
    if !path.exists() {
        eprintln!("emux: session '{}' not found.", session_name);
        std::process::exit(1);
    }
    run_attached(session_name)
}

/// `emux list` / `emux ls` — list sessions.
pub(crate) fn cmd_list() -> Result<(), AppError> {
    let sessions = list_live_sessions();
    if sessions.is_empty() {
        println!("No active sessions.");
    } else {
        // Try to get details from each daemon.
        for (name, path) in &sessions {
            if let Ok(mut client) = DaemonClient::connect(path)
                && client.send(ClientMessage::ListSessions).is_ok()
                && let Ok(ServerMessage::SessionList { sessions: entries }) = client.recv()
            {
                for entry in &entries {
                    println!(
                        "{}: {} tabs, {} panes ({}x{})",
                        entry.name, entry.tabs, entry.panes, entry.cols, entry.rows
                    );
                }
                continue;
            }
            // Fallback: just print the name.
            println!("{}", name);
        }
    }
    Ok(())
}

/// Parsed SSH subcommand from `emux ssh <dest> [subcmd] [args...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SshSubcommand {
    /// Attach to an existing (or default) remote session.
    Attach { session: Option<String> },
    /// Create a new remote session.
    New { session: Option<String> },
    /// List remote sessions.
    List,
}

/// Parsed result of `emux ssh <dest> ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SshArgs {
    pub domain: Domain,
    pub subcmd: SshSubcommand,
}

/// Parse the arguments after `emux ssh`.
///
/// Expected form: `emux ssh [user@]host[:port] [new [name] | attach [name] | list]`
pub(crate) fn parse_ssh_args(args: &[String]) -> Result<SshArgs, AppError> {
    if args.is_empty() {
        return Err("emux ssh: missing destination. Usage: emux ssh [user@]host[:port] [command]".into());
    }

    let domain = Domain::parse_remote(&args[0]).map_err(|e| {
        AppError::Msg(format!("emux ssh: invalid destination '{}': {e}", args[0]))
    })?;

    let subcmd = if args.len() < 2 {
        SshSubcommand::Attach { session: None }
    } else {
        match args[1].as_str() {
            "new" => SshSubcommand::New {
                session: args.get(2).cloned(),
            },
            "attach" | "a" => SshSubcommand::Attach {
                session: args.get(2).cloned(),
            },
            "list" | "ls" | "l" => SshSubcommand::List,
            other => {
                return Err(AppError::Msg(format!(
                    "emux ssh: unknown subcommand '{other}'. Try: new, attach, list"
                )));
            }
        }
    };

    Ok(SshArgs { domain, subcmd })
}

/// `emux ssh <dest> ...` — interact with a remote emux session.
pub(crate) fn cmd_ssh(args: &[String]) -> Result<(), AppError> {
    let ssh = parse_ssh_args(args)?;

    let dest = ssh
        .domain
        .ssh_destination()
        .expect("parse_ssh_args always returns Remote");

    match ssh.subcmd {
        SshSubcommand::Attach { ref session } => {
            let session_name = session.as_deref().unwrap_or("0");
            eprintln!("emux: connecting to {dest}, session '{session_name}'...");
            run_ssh_attach(&ssh.domain, session_name)
        }
        SshSubcommand::New { ref session } => {
            let session_name = session.as_deref().unwrap_or("0");
            eprintln!(
                "emux: creating session '{session_name}' on {dest}..."
            );
            run_ssh_command(&ssh.domain, &["new", session_name])
        }
        SshSubcommand::List => {
            run_ssh_command(&ssh.domain, &["list"])
        }
    }
}

/// Run an arbitrary emux command on the remote host via SSH.
fn run_ssh_command(domain: &Domain, emux_args: &[&str]) -> Result<(), AppError> {
    let Domain::Remote {
        host, user, port, ..
    } = domain
    else {
        return Err("not a remote domain".into());
    };

    let mut cmd = std::process::Command::new("ssh");
    cmd.arg("-t"); // allocate a PTY for interactive commands
    if let Some(p) = port {
        cmd.arg("-p").arg(p.to_string());
    }
    let destination = match user {
        Some(u) => format!("{u}@{host}"),
        None => host.clone(),
    };
    cmd.arg(&destination);
    cmd.arg("emux");
    for arg in emux_args {
        cmd.arg(arg);
    }

    let status = cmd.status().map_err(|e| {
        AppError::Msg(format!("failed to run ssh to {destination}: {e}"))
    })?;

    if !status.success() {
        return Err(AppError::Msg(format!(
            "ssh exited with status {status}"
        )));
    }
    Ok(())
}

/// Attach to a remote emux session via SSH with a PTY.
fn run_ssh_attach(domain: &Domain, session_name: &str) -> Result<(), AppError> {
    run_ssh_command(domain, &["attach", session_name])
}

/// `emux kill <name>` — kill a session.
pub(crate) fn cmd_kill(session_name: &str) -> Result<(), AppError> {
    let path = socket_path_for(session_name);
    if !path.exists() {
        eprintln!("emux: session '{}' not found.", session_name);
        std::process::exit(1);
    }
    match DaemonClient::connect(&path) {
        Ok(mut client) => {
            let _ = client.send(ClientMessage::KillSession {
                name: session_name.to_owned(),
            });
            // Read the Ack (best-effort).
            let _ = client.recv();
            println!("Session '{}' killed.", session_name);
        }
        Err(_) => {
            // Socket exists but nothing is listening — clean up.
            let _ = std::fs::remove_file(&path);
            println!("Cleaned up stale session '{}'.", session_name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn parse_ssh_args_host_only_defaults_to_attach() {
        let args = vec![s("myhost")];
        let ssh = parse_ssh_args(&args).unwrap();
        assert_eq!(
            ssh.domain,
            Domain::Remote {
                host: "myhost".into(),
                user: None,
                port: None
            }
        );
        assert_eq!(ssh.subcmd, SshSubcommand::Attach { session: None });
    }

    #[test]
    fn parse_ssh_args_user_host_port() {
        let args = vec![s("alice@server:2222"), s("new"), s("dev")];
        let ssh = parse_ssh_args(&args).unwrap();
        assert_eq!(
            ssh.domain,
            Domain::Remote {
                host: "server".into(),
                user: Some("alice".into()),
                port: Some(2222)
            }
        );
        assert_eq!(
            ssh.subcmd,
            SshSubcommand::New {
                session: Some("dev".into())
            }
        );
    }

    #[test]
    fn parse_ssh_args_attach_with_session() {
        let args = vec![s("bob@host"), s("attach"), s("main")];
        let ssh = parse_ssh_args(&args).unwrap();
        assert_eq!(
            ssh.subcmd,
            SshSubcommand::Attach {
                session: Some("main".into())
            }
        );
    }

    #[test]
    fn parse_ssh_args_attach_shorthand() {
        let args = vec![s("host"), s("a"), s("work")];
        let ssh = parse_ssh_args(&args).unwrap();
        assert_eq!(
            ssh.subcmd,
            SshSubcommand::Attach {
                session: Some("work".into())
            }
        );
    }

    #[test]
    fn parse_ssh_args_list() {
        let args = vec![s("host"), s("list")];
        let ssh = parse_ssh_args(&args).unwrap();
        assert_eq!(ssh.subcmd, SshSubcommand::List);
    }

    #[test]
    fn parse_ssh_args_list_shorthand_ls() {
        let args = vec![s("host"), s("ls")];
        let ssh = parse_ssh_args(&args).unwrap();
        assert_eq!(ssh.subcmd, SshSubcommand::List);
    }

    #[test]
    fn parse_ssh_args_list_shorthand_l() {
        let args = vec![s("host"), s("l")];
        let ssh = parse_ssh_args(&args).unwrap();
        assert_eq!(ssh.subcmd, SshSubcommand::List);
    }

    #[test]
    fn parse_ssh_args_new_no_name() {
        let args = vec![s("host"), s("new")];
        let ssh = parse_ssh_args(&args).unwrap();
        assert_eq!(ssh.subcmd, SshSubcommand::New { session: None });
    }

    #[test]
    fn parse_ssh_args_empty_fails() {
        let args: Vec<String> = vec![];
        assert!(parse_ssh_args(&args).is_err());
    }

    #[test]
    fn parse_ssh_args_invalid_destination_fails() {
        let args = vec![s("@host")]; // empty user
        assert!(parse_ssh_args(&args).is_err());
    }

    #[test]
    fn parse_ssh_args_unknown_subcommand_fails() {
        let args = vec![s("host"), s("frobnicate")];
        assert!(parse_ssh_args(&args).is_err());
    }

    #[test]
    fn parse_ssh_args_invalid_port_fails() {
        let args = vec![s("host:notaport")];
        assert!(parse_ssh_args(&args).is_err());
    }
}
