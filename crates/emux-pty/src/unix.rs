//! Unix PTY implementation using nix.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};

use nix::libc;
use nix::pty::{forkpty, ForkptyResult, Winsize};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{execvp, Pid};

use crate::cmdbuilder::CommandBuilder;

/// PTY dimensions.
#[derive(Debug, Clone, Copy)]
pub struct PtySize {
    /// Number of rows (lines) in the terminal.
    pub rows: u16,
    /// Number of columns (characters per line) in the terminal.
    pub cols: u16,
    /// Width of the terminal in pixels (0 if unknown).
    pub pixel_width: u16,
    /// Height of the terminal in pixels (0 if unknown).
    pub pixel_height: u16,
}

impl Default for PtySize {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

impl PtySize {
    fn to_winsize(self) -> Winsize {
        Winsize {
            ws_row: self.rows,
            ws_col: self.cols,
            ws_xpixel: self.pixel_width,
            ws_ypixel: self.pixel_height,
        }
    }
}

/// Errors that can occur when working with PTYs.
#[derive(Debug)]
pub enum PtyError {
    /// An I/O error occurred.
    Io(io::Error),
    /// A nix errno error occurred.
    Nix(nix::errno::Errno),
    /// The command or one of its arguments is invalid.
    InvalidCommand(String),
}

impl std::fmt::Display for PtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PtyError::Io(e) => write!(f, "I/O error: {e}"),
            PtyError::Nix(e) => write!(f, "nix error: {e}"),
            PtyError::InvalidCommand(msg) => write!(f, "invalid command: {msg}"),
        }
    }
}

impl std::error::Error for PtyError {}

impl From<io::Error> for PtyError {
    fn from(e: io::Error) -> Self {
        PtyError::Io(e)
    }
}

impl From<nix::errno::Errno> for PtyError {
    fn from(e: nix::errno::Errno) -> Self {
        PtyError::Nix(e)
    }
}

/// The exit status of a child process.
#[derive(Debug, Clone, Copy)]
pub enum ExitStatus {
    /// The process exited with the given code.
    Code(i32),
    /// The process was killed by the given signal.
    Signal(i32),
}

impl ExitStatus {
    /// Returns `true` if the process exited with code 0.
    pub fn success(&self) -> bool {
        matches!(self, ExitStatus::Code(0))
    }
}

impl std::fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitStatus::Code(c) => write!(f, "exit code {c}"),
            ExitStatus::Signal(s) => write!(f, "signal {s}"),
        }
    }
}

/// A Unix pseudo-terminal backed by a master fd and a child process.
pub struct UnixPty {
    master: File,
    child_pid: Pid,
}

impl UnixPty {
    /// Spawn a child process inside a new PTY.
    ///
    /// This uses `forkpty` which handles:
    /// - Creating the master/slave PTY pair
    /// - Forking the process
    /// - In the child: calling `setsid()`, setting the controlling terminal,
    ///   and duping the slave fd onto stdin/stdout/stderr
    /// - In the parent: returning the master fd
    pub fn spawn(cmd: &CommandBuilder, size: PtySize) -> Result<Self, PtyError> {
        let winsize = size.to_winsize();

        // Safety: forkpty is unsafe because of the fork. We only call
        // async-signal-safe operations (chdir, execvp) in the child.
        let result = unsafe { forkpty(&winsize, None)? };

        match result {
            ForkptyResult::Parent { child, master } => {
                let raw_fd = master.into_raw_fd();
                let master_file = unsafe { File::from_raw_fd(raw_fd) };
                Ok(UnixPty {
                    master: master_file,
                    child_pid: child,
                })
            }
            ForkptyResult::Child => {
                // We are in the child process.
                // The child must never return — wrap all fallible operations
                // in a closure so that errors cause `_exit(127)` instead of
                // unwinding back into the parent's code path.
                let child_result = (|| -> Result<(), PtyError> {
                    // Apply environment variables.
                    for (key, val) in cmd.env_map() {
                        // Safety: we are in a single-threaded child process right
                        // after fork, so mutating the environment is fine.
                        unsafe { std::env::set_var(key, val) };
                    }

                    // Change working directory if specified.
                    if let Some(cwd) = cmd.cwd_path()
                        && std::env::set_current_dir(cwd).is_err() {
                            // If we can't change directory, just continue with the
                            // inherited cwd rather than failing silently.
                        }

                    // Set TERM if not already set by the caller.
                    if !cmd.env_map().contains_key("TERM") {
                        unsafe { std::env::set_var("TERM", "xterm-256color") };
                    }

                    // Build argv and exec.
                    let argv = cmd.argv_cstrings()?;
                    let program = cmd.program_cstr()?;

                    // execvp never returns on success.
                    let _ = execvp(&program, &argv);

                    Ok(())
                })();

                // If anything failed (including execvp returning), exit the
                // child immediately. Use _exit to avoid running atexit handlers.
                if child_result.is_err() {
                    unsafe { libc::_exit(127) };
                }
                unsafe { libc::_exit(127) };
            }
        }
    }

    /// Read bytes from the master side of the PTY.
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.master.read(buf)
    }

    /// Write bytes to the master side of the PTY (i.e. send input to the child).
    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.master.write(buf)
    }

    /// Resize the PTY window.
    pub fn resize(&self, size: PtySize) -> Result<(), PtyError> {
        let ws = size.to_winsize();
        let fd = self.master.as_raw_fd();
        let ret = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &raw const ws) };
        if ret < 0 {
            return Err(PtyError::Io(io::Error::last_os_error()));
        }
        Ok(())
    }

    /// Return the child process PID.
    pub fn child_pid(&self) -> u32 {
        self.child_pid.as_raw() as u32
    }

    /// Wait for the child process to exit, returning its status.
    pub fn wait(&self) -> Result<ExitStatus, PtyError> {
        match waitpid(self.child_pid, None)? {
            WaitStatus::Exited(_, code) => Ok(ExitStatus::Code(code)),
            WaitStatus::Signaled(_, sig, _) => Ok(ExitStatus::Signal(sig as i32)),
            _ => Ok(ExitStatus::Code(-1)),
        }
    }

    /// Check if the child process is still alive (non-blocking).
    pub fn is_alive(&self) -> bool {
        matches!(waitpid(self.child_pid, Some(WaitPidFlag::WNOHANG)), Ok(WaitStatus::StillAlive))
    }

    /// Return a reference to the underlying master file.
    pub fn master_file(&self) -> &File {
        &self.master
    }

    /// Return the raw fd of the master side.
    pub fn master_raw_fd(&self) -> i32 {
        self.master.as_raw_fd()
    }
}

impl Read for UnixPty {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.master.read(buf)
    }
}

impl Write for UnixPty {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.master.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.master.flush()
    }
}

impl Drop for UnixPty {
    fn drop(&mut self) {
        // Attempt to kill the child process if it's still running.
        unsafe { libc::kill(self.child_pid.as_raw(), libc::SIGHUP) };
    }
}
