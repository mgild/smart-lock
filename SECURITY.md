# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly:

1. **Do not** open a public GitHub issue
2. Email the maintainer or use [GitHub's private vulnerability reporting](https://github.com/mgild/smart-lock/security/advisories/new)
3. Include a description of the vulnerability, steps to reproduce, and potential impact

We will acknowledge receipt within 48 hours and provide a timeline for a fix.

## Scope

smart-lock contains **zero `unsafe` code**. The crate delegates all lock operations to [`async-lock`](https://docs.rs/async-lock), which is widely audited. The proc macro generates safe Rust code only.

Potential security concerns:
- Denial of service via deadlock (mitigated by field-order acquisition)
- Logic errors in generated code leading to unsound access patterns

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |
