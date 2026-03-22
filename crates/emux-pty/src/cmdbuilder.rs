//! Command builder for spawning shell processes.

use std::collections::HashMap;
#[cfg(unix)]
use std::ffi::CString;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use crate::unix::PtyError;
#[cfg(windows)]
use crate::windows::PtyError;

/// Builds the command and environment for a PTY child process.
#[derive(Debug, Clone)]
pub struct CommandBuilder {
    program: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    cwd: Option<PathBuf>,
}

impl CommandBuilder {
    /// Create a new command builder for the given program.
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: None,
        }
    }

    /// Add an argument to the command.
    pub fn arg(&mut self, arg: &str) -> &mut Self {
        self.args.push(arg.to_string());
        self
    }

    /// Set an environment variable.
    pub fn env(&mut self, key: &str, val: &str) -> &mut Self {
        self.env.insert(key.to_string(), val.to_string());
        self
    }

    /// Set the working directory for the child process.
    pub fn cwd(&mut self, dir: &Path) -> &mut Self {
        self.cwd = Some(dir.to_path_buf());
        self
    }

    /// Create a command builder that launches the user's default shell.
    ///
    /// On Unix, uses the `$SHELL` environment variable, falling back to `/bin/sh`.
    /// On Windows, uses `%COMSPEC%`, falling back to `cmd.exe`.
    pub fn default_shell() -> Self {
        #[cfg(unix)]
        {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            Self::new(&shell)
        }
        #[cfg(windows)]
        {
            let shell = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
            Self::new(&shell)
        }
    }

    /// Return the program name.
    pub fn program(&self) -> &str {
        &self.program
    }

    /// Return the argument list.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Return the environment overrides.
    pub fn env_map(&self) -> &HashMap<String, String> {
        &self.env
    }

    /// Return the working directory, if set.
    pub fn cwd_path(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    /// Build a `CString` for the program name.
    #[cfg(unix)]
    pub(crate) fn program_cstr(&self) -> Result<CString, PtyError> {
        CString::new(self.program.as_str())
            .map_err(|_| PtyError::InvalidCommand("program contains nul byte".into()))
    }

    /// Build a vector of `CString` arguments (argv), with program as argv[0].
    #[cfg(unix)]
    pub(crate) fn argv_cstrings(&self) -> Result<Vec<CString>, PtyError> {
        let mut argv = Vec::with_capacity(1 + self.args.len());
        argv.push(self.program_cstr()?);
        for arg in &self.args {
            argv.push(
                CString::new(arg.as_str())
                    .map_err(|_| PtyError::InvalidCommand("arg contains nul byte".into()))?,
            );
        }
        Ok(argv)
    }
}
