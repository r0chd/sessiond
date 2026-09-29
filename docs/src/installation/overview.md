# Installation

Choose the guide for your system. Each covers installation and platform-specific setup:

- [Finix](finix.md): native integration and PAM configuration.
- [NixOS](nixos.md): module installation, logind, PAM, and Polkit integration.

## From source

Build `sessiond`, `sessionctl`, and the PAM module in release mode:

```bash
cargo build -p sessiond -p sessionctl -p pam --release
```

Install the binaries and PAM module:

```bash
install -Dm755 target/release/sessiond /usr/local/bin/sessiond
install -Dm755 target/release/sessionctl /usr/local/bin/sessionctl
install -Dm755 target/release/libpam.so /usr/lib/security/pam_sessiond.so
```

The `consolekit` feature enables the ConsoleKit compatibility layer. Include it
when building if you need that support:

```bash
cargo build -p sessiond --release --features consolekit
```
