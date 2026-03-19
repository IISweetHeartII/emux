//! Windows ConPTY implementation.

#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::{self, Read, Write};
#[cfg(windows)]
use std::mem;
#[cfg(windows)]
use std::os::windows::io::FromRawHandle;
#[cfg(windows)]
use std::ptr;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{BOOL, CloseHandle, HANDLE, INVALID_HANDLE_VALUE, S_OK, TRUE};
#[cfg(windows)]
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
#[cfg(windows)]
use windows_sys::Win32::System::Console::{
    COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, ResizePseudoConsole,
};
#[cfg(windows)]
use windows_sys::Win32::System::Pipes::CreatePipe;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
    GetExitCodeProcess, InitializeProcThreadAttributeList, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
    PROCESS_INFORMATION, STARTUPINFOEXW, UpdateProcThreadAttribute,
};

#[cfg(windows)]
use crate::cmdbuilder::CommandBuilder;

/// PTY dimensions.
#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
pub struct PtySize {
    pub rows: u16,
    pub cols: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

#[cfg(windows)]
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

/// Errors that can occur when working with PTYs.
#[cfg(windows)]
#[derive(Debug)]
pub enum PtyError {
    /// An I/O error occurred.
    Io(io::Error),
    /// A Windows API call failed.
    Win32(String),
    /// The command or one of its arguments is invalid.
    InvalidCommand(String),
}

#[cfg(windows)]
impl std::fmt::Display for PtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PtyError::Io(e) => write!(f, "I/O error: {e}"),
            PtyError::Win32(msg) => write!(f, "Win32 error: {msg}"),
            PtyError::InvalidCommand(msg) => write!(f, "invalid command: {msg}"),
        }
    }
}

#[cfg(windows)]
impl std::error::Error for PtyError {}

#[cfg(windows)]
impl From<io::Error> for PtyError {
    fn from(e: io::Error) -> Self {
        PtyError::Io(e)
    }
}

/// The exit status of a child process.
#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
pub enum ExitStatus {
    /// The process exited with the given code.
    Code(i32),
}

#[cfg(windows)]
impl ExitStatus {
    /// Returns `true` if the process exited with code 0.
    pub fn success(&self) -> bool {
        matches!(self, ExitStatus::Code(0))
    }
}

#[cfg(windows)]
impl std::fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitStatus::Code(c) => write!(f, "exit code {c}"),
        }
    }
}

/// A Windows pseudo-terminal backed by ConPTY.
#[cfg(windows)]
pub struct WinPty {
    /// ConPTY handle.
    pty_handle: HPCON,
    /// Write to this pipe to send input to the child's stdin.
    input_write: File,
    /// Read from this pipe to receive output from the child's stdout.
    output_read: File,
    /// Child process handle.
    process_handle: HANDLE,
    /// Child thread handle.
    thread_handle: HANDLE,
    /// Child process ID.
    pid: u32,
}

// HANDLE values are Send-safe (they are kernel object references).
#[cfg(windows)]
unsafe impl Send for WinPty {}

#[cfg(windows)]
impl WinPty {
    /// Spawn a child process inside a new ConPTY.
    pub fn spawn(cmd: &CommandBuilder, size: PtySize) -> Result<Self, PtyError> {
        unsafe {
            // 1. Create pipes for PTY I/O.
            let mut input_read: HANDLE = INVALID_HANDLE_VALUE;
            let mut input_write: HANDLE = INVALID_HANDLE_VALUE;
            let mut output_read: HANDLE = INVALID_HANDLE_VALUE;
            let mut output_write: HANDLE = INVALID_HANDLE_VALUE;

            if CreatePipe(&mut input_read, &mut input_write, ptr::null(), 0) == 0 {
                return Err(PtyError::Io(io::Error::last_os_error()));
            }
            if CreatePipe(&mut output_read, &mut output_write, ptr::null(), 0) == 0 {
                CloseHandle(input_read);
                CloseHandle(input_write);
                return Err(PtyError::Io(io::Error::last_os_error()));
            }

            // 2. Create the pseudo console.
            let coord = COORD {
                X: size.cols as i16,
                Y: size.rows as i16,
            };
            let mut pty_handle: HPCON = 0;
            let hr = CreatePseudoConsole(coord, input_read, output_write, 0, &mut pty_handle);
            if hr != S_OK {
                CloseHandle(input_read);
                CloseHandle(input_write);
                CloseHandle(output_read);
                CloseHandle(output_write);
                return Err(PtyError::Win32(format!(
                    "CreatePseudoConsole failed: HRESULT 0x{hr:08X}"
                )));
            }

            // Close the child-side handles — the pseudo console owns them now.
            CloseHandle(input_read);
            CloseHandle(output_write);

            // 3. Initialize thread attribute list with the pseudo console.
            let mut attr_list_size: usize = 0;
            InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut attr_list_size);
            let mut attr_list_buf: Vec<u8> = vec![0u8; attr_list_size];
            let attr_list = attr_list_buf.as_mut_ptr() as *mut _;

            if InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_list_size) == 0 {
                ClosePseudoConsole(pty_handle);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(PtyError::Io(io::Error::last_os_error()));
            }

            if UpdateProcThreadAttribute(
                attr_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                pty_handle as *const _,
                mem::size_of::<HPCON>(),
                ptr::null_mut(),
                ptr::null(),
            ) == 0
            {
                DeleteProcThreadAttributeList(attr_list);
                ClosePseudoConsole(pty_handle);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(PtyError::Io(io::Error::last_os_error()));
            }

            // 4. Build the command line as a wide string.
            let command_line = Self::build_command_line(cmd);
            let mut command_line_w: Vec<u16> = command_line
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            // Build environment block if needed.
            let env_block = if !cmd.env_map().is_empty() {
                Some(Self::build_env_block(cmd))
            } else {
                None
            };

            // Build working directory as wide string.
            let cwd_w: Option<Vec<u16>> = cmd.cwd_path().map(|p| {
                p.to_string_lossy()
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect()
            });

            // 5. Set up STARTUPINFOEXW.
            let mut si: STARTUPINFOEXW = mem::zeroed();
            si.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
            si.lpAttributeList = attr_list;

            let mut pi: PROCESS_INFORMATION = mem::zeroed();

            let result = CreateProcessW(
                ptr::null(),
                command_line_w.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0, // bInheritHandles = FALSE
                EXTENDED_STARTUPINFO_PRESENT,
                env_block
                    .as_ref()
                    .map(|b| b.as_ptr() as *const _)
                    .unwrap_or(ptr::null()),
                cwd_w.as_ref().map(|c| c.as_ptr()).unwrap_or(ptr::null()),
                &si.StartupInfo,
                &mut pi,
            );

            DeleteProcThreadAttributeList(attr_list);

            if result == 0 {
                ClosePseudoConsole(pty_handle);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(PtyError::Io(io::Error::last_os_error()));
            }

            let pid = pi.dwProcessId;

            Ok(WinPty {
                pty_handle,
                input_write: File::from_raw_handle(input_write as *mut _),
                output_read: File::from_raw_handle(output_read as *mut _),
                process_handle: pi.hProcess,
                thread_handle: pi.hThread,
                pid,
            })
        }
    }

    /// Resize the PTY window.
    pub fn resize(&self, size: PtySize) -> Result<(), PtyError> {
        let coord = COORD {
            X: size.cols as i16,
            Y: size.rows as i16,
        };
        let hr = unsafe { ResizePseudoConsole(self.pty_handle, coord) };
        if hr != S_OK {
            return Err(PtyError::Win32(format!(
                "ResizePseudoConsole failed: HRESULT 0x{hr:08X}"
            )));
        }
        Ok(())
    }

    /// Return the child process PID.
    pub fn child_pid(&self) -> u32 {
        self.pid
    }

    /// Check if the child process is still alive (non-blocking).
    pub fn is_alive(&self) -> bool {
        unsafe {
            let mut exit_code: u32 = 0;
            if GetExitCodeProcess(self.process_handle, &mut exit_code) != 0 {
                // STILL_ACTIVE == 259
                exit_code == 259
            } else {
                false
            }
        }
    }

    /// Wait for the child process to exit, returning its status.
    pub fn wait(&mut self) -> Result<ExitStatus, PtyError> {
        unsafe {
            use windows_sys::Win32::System::Threading::WaitForSingleObject;
            WaitForSingleObject(self.process_handle, 0xFFFFFFFF); // INFINITE
            let mut exit_code: u32 = 0;
            if GetExitCodeProcess(self.process_handle, &mut exit_code) == 0 {
                return Err(PtyError::Io(io::Error::last_os_error()));
            }
            Ok(ExitStatus::Code(exit_code as i32))
        }
    }

    /// Build a command line string from a CommandBuilder.
    fn build_command_line(cmd: &CommandBuilder) -> String {
        let mut line = Self::quote_arg(cmd.program());
        for arg in cmd.args() {
            line.push(' ');
            line.push_str(&Self::quote_arg(arg));
        }
        line
    }

    /// Quote a command-line argument for Windows CreateProcessW.
    fn quote_arg(arg: &str) -> String {
        if arg.is_empty() {
            return "\"\"".to_string();
        }
        if !arg.contains(' ') && !arg.contains('"') && !arg.contains('\t') {
            return arg.to_string();
        }
        let mut quoted = String::with_capacity(arg.len() + 2);
        quoted.push('"');
        let mut backslashes = 0;
        for c in arg.chars() {
            if c == '\\' {
                backslashes += 1;
            } else if c == '"' {
                // Double the backslashes before a quote.
                for _ in 0..backslashes {
                    quoted.push('\\');
                }
                backslashes = 0;
                quoted.push('\\');
                quoted.push('"');
            } else {
                backslashes = 0;
                quoted.push(c);
            }
        }
        // Double trailing backslashes before the closing quote.
        for _ in 0..backslashes {
            quoted.push('\\');
        }
        quoted.push('"');
        quoted
    }

    /// Build a Windows environment block (null-separated, double-null terminated)
    /// by merging the current process environment with the overrides from CommandBuilder.
    fn build_env_block(cmd: &CommandBuilder) -> Vec<u8> {
        use std::collections::BTreeMap;

        // Start with the current environment.
        let mut env: BTreeMap<String, String> = std::env::vars().collect();

        // Apply overrides.
        for (k, v) in cmd.env_map() {
            env.insert(k.clone(), v.clone());
        }

        // Build the block: each entry is "KEY=VALUE\0", terminated by an extra \0.
        let mut block = Vec::new();
        for (k, v) in &env {
            let entry = format!("{k}={v}");
            block.extend(entry.as_bytes());
            block.push(0);
        }
        block.push(0);
        block
    }
}

#[cfg(windows)]
impl Read for WinPty {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.output_read.read(buf)
    }
}

#[cfg(windows)]
impl Write for WinPty {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.input_write.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.input_write.flush()
    }
}

#[cfg(windows)]
impl Drop for WinPty {
    fn drop(&mut self) {
        unsafe {
            // Close the pseudo console first — this signals the child.
            ClosePseudoConsole(self.pty_handle);
            // Clean up process and thread handles.
            CloseHandle(self.process_handle);
            CloseHandle(self.thread_handle);
        }
    }
}

// ---------------------------------------------------------------------------
// Compile-checked tests (only compiled on Windows, never run on other platforms)
// ---------------------------------------------------------------------------

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::cmdbuilder::CommandBuilder;

    #[test]
    fn spawn_cmd_exe() {
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.arg("/C").arg("echo hello");
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };
        let mut pty = WinPty::spawn(&cmd, size).expect("failed to spawn cmd.exe");
        let mut buf = [0u8; 1024];
        // Read some output.
        let n = pty.read(&mut buf).expect("failed to read");
        assert!(n > 0);
    }

    #[test]
    fn resize_pty() {
        let cmd = CommandBuilder::new("cmd.exe");
        let size = PtySize::default();
        let pty = WinPty::spawn(&cmd, size).expect("failed to spawn");
        let new_size = PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        };
        pty.resize(new_size).expect("resize failed");
    }

    #[test]
    fn child_pid_nonzero() {
        let cmd = CommandBuilder::new("cmd.exe");
        let size = PtySize::default();
        let pty = WinPty::spawn(&cmd, size).expect("failed to spawn");
        assert!(pty.child_pid() > 0);
    }

    #[test]
    fn is_alive_after_spawn() {
        let cmd = CommandBuilder::new("cmd.exe");
        let size = PtySize::default();
        let pty = WinPty::spawn(&cmd, size).expect("failed to spawn");
        assert!(pty.is_alive());
    }

    #[test]
    fn wait_for_exit() {
        let mut cmd = CommandBuilder::new("cmd.exe");
        cmd.arg("/C").arg("exit 0");
        let size = PtySize::default();
        let mut pty = WinPty::spawn(&cmd, size).expect("failed to spawn");
        let status = pty.wait().expect("wait failed");
        assert!(status.success());
    }
}
