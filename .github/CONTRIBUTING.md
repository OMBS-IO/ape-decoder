# Contributing to ape-decoder

Thanks for your interest in contributing! This guide covers how to get set up and submit changes.

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- Git

### Setup

```bash
git clone https://github.com/ombs-io/ape-decoder.git
cd ape-decoder
cargo build
cargo test
```

### Useful Commands

```bash
cargo test              # Run all tests
cargo clippy            # Lint checks
cargo fmt               # Format code
cargo doc --open        # Build and view documentation
```

## Project Structure

| Directory | Purpose |
|-----------|---------|
| `src/` | Core decoder implementation |
| `tests/` | Integration tests |
| `docs/` | Algorithm documentation from the C++ reference |

## How to Submit Changes

1. Fork the repository and create a branch from `main`.
2. Make your changes, adding tests for new behavior.
3. Ensure `cargo test`, `cargo clippy`, and `cargo fmt --check` all pass.
4. Open a pull request with a clear description of the change.

## Code Style

- Follow standard Rust conventions and `rustfmt` defaults.
- Write doc comments for public API items.
- Keep unsafe code to an absolute minimum and document why it's necessary.

## Reporting Bugs

Use the [bug report template](https://github.com/ombs-io/ape-decoder/issues/new?template=bug_report.md) to file issues. Include APE file details and reproduction steps when possible.

## Security

If you discover a security vulnerability, **do not** open a public issue. See [SECURITY.md](SECURITY.md) for reporting instructions.
