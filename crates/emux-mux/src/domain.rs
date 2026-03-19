//! Domain abstraction for local and remote sessions.
//!
//! A [`Domain`] describes where a session lives: either on the local machine
//! or on a remote host reachable via SSH.

use std::fmt;

/// Identifies the execution domain of a session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Domain {
    /// The session runs on the local machine.
    #[default]
    Local,
    /// The session runs on a remote host.
    Remote {
        host: String,
        user: Option<String>,
        port: Option<u16>,
    },
}

impl Domain {
    /// Returns `true` if this is a local domain.
    pub fn is_local(&self) -> bool {
        matches!(self, Domain::Local)
    }

    /// Returns `true` if this is a remote domain.
    pub fn is_remote(&self) -> bool {
        matches!(self, Domain::Remote { .. })
    }

    /// Build an SSH destination string suitable for passing to `ssh`.
    ///
    /// Returns `None` for local domains.
    /// For remote domains the format is `[user@]host[:port]`.
    pub fn ssh_destination(&self) -> Option<String> {
        match self {
            Domain::Local => None,
            Domain::Remote { host, user, port } => {
                let mut dest = String::new();
                if let Some(u) = user {
                    dest.push_str(u);
                    dest.push('@');
                }
                dest.push_str(host);
                if let Some(p) = port {
                    dest.push(':');
                    dest.push_str(&p.to_string());
                }
                Some(dest)
            }
        }
    }

    /// Construct a remote domain from an SSH-style destination string.
    ///
    /// Accepted formats:
    /// - `host`
    /// - `user@host`
    /// - `host:port`
    /// - `user@host:port`
    pub fn parse_remote(s: &str) -> Result<Self, DomainParseError> {
        if s.is_empty() {
            return Err(DomainParseError::EmptyHost);
        }

        let (user, rest) = if let Some(at_pos) = s.find('@') {
            let user = &s[..at_pos];
            if user.is_empty() {
                return Err(DomainParseError::EmptyUser);
            }
            (Some(user.to_string()), &s[at_pos + 1..])
        } else {
            (None, s)
        };

        let (host, port) = if let Some(colon_pos) = rest.rfind(':') {
            let host = &rest[..colon_pos];
            let port_str = &rest[colon_pos + 1..];
            let port = port_str
                .parse::<u16>()
                .map_err(|_| DomainParseError::InvalidPort(port_str.to_string()))?;
            (host.to_string(), Some(port))
        } else {
            (rest.to_string(), None)
        };

        if host.is_empty() {
            return Err(DomainParseError::EmptyHost);
        }

        Ok(Domain::Remote { host, user, port })
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Domain::Local => write!(f, "local"),
            Domain::Remote { host, user, port } => {
                if let Some(u) = user {
                    write!(f, "{u}@")?;
                }
                write!(f, "{host}")?;
                if let Some(p) = port {
                    write!(f, ":{p}")?;
                }
                Ok(())
            }
        }
    }
}

/// Errors returned by [`Domain::parse_remote`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainParseError {
    EmptyHost,
    EmptyUser,
    InvalidPort(String),
}

impl fmt::Display for DomainParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainParseError::EmptyHost => write!(f, "host cannot be empty"),
            DomainParseError::EmptyUser => write!(f, "user cannot be empty"),
            DomainParseError::InvalidPort(s) => write!(f, "invalid port: {s}"),
        }
    }
}

impl std::error::Error for DomainParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_domain() {
        let d = Domain::Local;
        assert!(d.is_local());
        assert!(!d.is_remote());
        assert_eq!(d.ssh_destination(), None);
        assert_eq!(d.to_string(), "local");
    }

    #[test]
    fn remote_domain_host_only() {
        let d = Domain::Remote {
            host: "example.com".into(),
            user: None,
            port: None,
        };
        assert!(!d.is_local());
        assert!(d.is_remote());
        assert_eq!(d.ssh_destination(), Some("example.com".into()));
        assert_eq!(d.to_string(), "example.com");
    }

    #[test]
    fn remote_domain_full() {
        let d = Domain::Remote {
            host: "example.com".into(),
            user: Some("alice".into()),
            port: Some(2222),
        };
        assert_eq!(d.ssh_destination(), Some("alice@example.com:2222".into()));
        assert_eq!(d.to_string(), "alice@example.com:2222");
    }

    #[test]
    fn remote_domain_user_no_port() {
        let d = Domain::Remote {
            host: "server.local".into(),
            user: Some("bob".into()),
            port: None,
        };
        assert_eq!(d.ssh_destination(), Some("bob@server.local".into()));
    }

    #[test]
    fn remote_domain_port_no_user() {
        let d = Domain::Remote {
            host: "10.0.0.1".into(),
            user: None,
            port: Some(22),
        };
        assert_eq!(d.ssh_destination(), Some("10.0.0.1:22".into()));
    }

    #[test]
    fn default_is_local() {
        assert_eq!(Domain::default(), Domain::Local);
    }

    #[test]
    fn parse_remote_host_only() {
        let d = Domain::parse_remote("example.com").unwrap();
        assert_eq!(
            d,
            Domain::Remote {
                host: "example.com".into(),
                user: None,
                port: None,
            }
        );
    }

    #[test]
    fn parse_remote_user_host() {
        let d = Domain::parse_remote("alice@example.com").unwrap();
        assert_eq!(
            d,
            Domain::Remote {
                host: "example.com".into(),
                user: Some("alice".into()),
                port: None,
            }
        );
    }

    #[test]
    fn parse_remote_host_port() {
        let d = Domain::parse_remote("example.com:2222").unwrap();
        assert_eq!(
            d,
            Domain::Remote {
                host: "example.com".into(),
                user: None,
                port: Some(2222),
            }
        );
    }

    #[test]
    fn parse_remote_full() {
        let d = Domain::parse_remote("alice@example.com:2222").unwrap();
        assert_eq!(
            d,
            Domain::Remote {
                host: "example.com".into(),
                user: Some("alice".into()),
                port: Some(2222),
            }
        );
    }

    #[test]
    fn parse_remote_empty_fails() {
        assert_eq!(Domain::parse_remote(""), Err(DomainParseError::EmptyHost));
    }

    #[test]
    fn parse_remote_empty_user_fails() {
        assert_eq!(
            Domain::parse_remote("@host"),
            Err(DomainParseError::EmptyUser)
        );
    }

    #[test]
    fn parse_remote_invalid_port() {
        assert!(matches!(
            Domain::parse_remote("host:abc"),
            Err(DomainParseError::InvalidPort(_))
        ));
    }

    #[test]
    fn domain_equality() {
        let a = Domain::Remote {
            host: "h".into(),
            user: Some("u".into()),
            port: Some(22),
        };
        let b = a.clone();
        assert_eq!(a, b);
        assert_ne!(a, Domain::Local);
    }
}
