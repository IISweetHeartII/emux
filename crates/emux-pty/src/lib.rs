//! Pseudo-terminal abstraction layer (unix and windows).

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;

pub mod cmdbuilder;

pub use cmdbuilder::CommandBuilder;

#[cfg(unix)]
pub use unix::{ExitStatus, PtyError, PtySize, UnixPty};

#[cfg(windows)]
pub use windows::{ExitStatus, PtyError, PtySize, WinPty};

/// Platform-specific PTY type alias.
#[cfg(unix)]
pub type NativePty = UnixPty;

/// Platform-specific PTY type alias.
#[cfg(windows)]
pub type NativePty = WinPty;

use std::io::{Read, Write};

/// Trait for interacting with a pseudo-terminal.
pub trait Pty: Read + Write {
    /// Resize the PTY to the given dimensions.
    fn resize(&self, size: PtySize) -> Result<(), PtyError>;

    /// Return the PID of the child process.
    fn child_pid(&self) -> u32;

    /// Check whether the child process is still running.
    fn is_alive(&self) -> bool;
}

#[cfg(unix)]
impl Pty for UnixPty {
    fn resize(&self, size: PtySize) -> Result<(), PtyError> {
        self.resize(size)
    }

    fn child_pid(&self) -> u32 {
        self.child_pid()
    }

    fn is_alive(&self) -> bool {
        self.is_alive()
    }
}

#[cfg(windows)]
impl Pty for WinPty {
    fn resize(&self, size: PtySize) -> Result<(), PtyError> {
        self.resize(size)
    }

    fn child_pid(&self) -> u32 {
        self.child_pid()
    }

    fn is_alive(&self) -> bool {
        self.is_alive()
    }
}
