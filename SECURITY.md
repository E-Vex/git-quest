# Security Policy

## Reporting a Vulnerability

If you find a security issue, please open a private security advisory on GitHub or email the maintainer directly. Do not open public issues for security vulnerabilities.

## Token Safety

`git-quest` reads `GITHUB_TOKEN` from the environment but never writes it to files or logs. If you believe a token has been exposed in a log or file, revoke it immediately at https://github.com/settings/tokens.
