# Building Cerberus

## Supported target

The repository pins the stable Rust MSVC toolchain and `x86_64-pc-windows-msvc` target in `rust-toolchain.toml`. The current desktop application is intended for 64-bit Windows.

## Prerequisites

- Rust and rustup from [rustup.rs](https://rustup.rs/).
- Visual Studio C++ Build Tools, including the MSVC compiler and Windows SDK.
- Git, if cloning the repository.

The pinned Rust toolchain is installed automatically by rustup when first used. Install the C++ tools separately; MinGW's `dlltool` is not a substitute for the MSVC target linker/import-library tools.

## Build commands

From the repository root in PowerShell:

```powershell
# Optimized release build
./build.ps1

# Or use Cargo directly
cargo build --release --target x86_64-pc-windows-msvc

# Run a development build
cargo run --target x86_64-pc-windows-msvc
```

The release executable is `target/x86_64-pc-windows-msvc/release/cerberus.exe`. The `target/` directory contains generated local build output and is ignored by Git.

## Verification commands

```powershell
cargo fmt --all -- --check
cargo test --locked --target x86_64-pc-windows-msvc
cargo clippy --all-targets --locked --target x86_64-pc-windows-msvc -- -D warnings
```
