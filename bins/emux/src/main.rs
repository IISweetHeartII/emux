//! emux — a terminal multiplexer with multiple panes, tabs, and keybindings.

#[macro_use]
mod logging;
mod app;
mod cli;
mod daemon;
mod error;
mod event_loop;
mod input;
mod keybindings;
mod operations;
mod render;

pub use error::AppError;

use cli::{cmd_attach, cmd_default, cmd_kill, cmd_list, cmd_new, cmd_ssh, generate_session_name, print_help};
use daemon::list_live_sessions;
use logging::init_logging;

fn main() -> Result<(), AppError> {
    init_logging();
    emux_log!("emux starting, args: {:?}", std::env::args().collect::<Vec<_>>());

    let args: Vec<String> = std::env::args().collect();

    // Detect nested sessions (like tmux's $TMUX check).
    // Allow `emux ls`, `emux kill`, etc. but block new sessions.
    if std::env::var("EMUX").is_ok() {
        let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("");
        match cmd {
            "list" | "ls" | "l" | "kill" | "ssh" | "--help" | "-h" | "--version" | "-V" => {
                // These are safe inside emux — allow them.
            }
            _ => {
                eprintln!("emux: sessions should be nested with care, unset $EMUX to force");
                std::process::exit(1);
            }
        }
    }

    if args.len() > 1 {
        match args[1].as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--version" | "-V" => {
                println!("emux {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "new" => {
                let session_name = if args.len() > 2 {
                    args[2].clone()
                } else {
                    generate_session_name()
                };
                return cmd_new(&session_name);
            }
            "attach" | "a" => {
                let session_name = if args.len() > 2 {
                    args[2].clone()
                } else {
                    // Attach to the first available session.
                    let sessions = list_live_sessions();
                    if sessions.is_empty() {
                        eprintln!("emux: no sessions to attach to. Use 'emux new' to create one.");
                        std::process::exit(1);
                    }
                    sessions[0].0.clone()
                };
                return cmd_attach(&session_name);
            }
            "list" | "ls" | "l" => {
                return cmd_list();
            }
            "kill" => {
                if args.len() < 3 {
                    eprintln!("emux: 'kill' requires a session name. Try 'emux kill <name>'.");
                    std::process::exit(1);
                }
                return cmd_kill(&args[2]);
            }
            "ssh" => {
                return cmd_ssh(&args[2..]);
            }
            other if other.starts_with('-') => {
                eprintln!("emux: unknown option '{}'. Try 'emux --help'.", other);
                std::process::exit(1);
            }
            other => {
                eprintln!("emux: unknown command '{}'. Try 'emux --help'.", other);
                std::process::exit(1);
            }
        }
    }

    // No arguments: try to attach to an existing session, or start a new one.
    cmd_default()
}
