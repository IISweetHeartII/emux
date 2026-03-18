# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security seriously at emux. If you discover a security vulnerability,
please report it responsibly.

### How to Report

**DO NOT** open a public GitHub issue for security vulnerabilities.

Instead, please use [GitHub Security Advisories](https://github.com/IISweetHeartII/emux/security/advisories/new) to report vulnerabilities privately.

Include the following in your report:

1. **Description** of the vulnerability
2. **Steps to reproduce** the issue
3. **Potential impact** of the vulnerability
4. **Suggested fix** (if any)

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial Assessment**: Within 5 business days
- **Fix & Disclosure**: We aim to patch critical vulnerabilities within 14 days

### Scope

The following are in scope:

- emux binary and all workspace crates
- PTY handling and process management
- IPC protocol and daemon communication
- Session persistence and state management
- Configuration file parsing

### Out of Scope

- Denial of service attacks
- Social engineering
- Issues in third-party dependencies (report to upstream)
- Issues requiring physical access to the machine

## Security Best Practices for Contributors

When contributing to emux:

1. **No unsafe without justification** -- Every `unsafe` block must have a `// SAFETY:` comment
2. **Validate all IPC inputs** -- Never trust data from the IPC protocol without validation
3. **Handle PTY errors gracefully** -- PTY operations can fail; don't panic
4. **Avoid path traversal** -- Validate file paths in configuration loading
5. **Memory safety** -- Leverage Rust's ownership system; avoid raw pointers when possible

## Hall of Fame

We gratefully acknowledge security researchers who help keep emux safe.
Contributors will be listed here (with permission).
