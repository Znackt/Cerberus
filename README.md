# Cerberus

Cerberus is an early-stage Windows desktop application foundation for managing cloud-hosted game files through a local storage workflow. It is written in Rust and uses egui for its native desktop interface.

> **Status:** The current build provides the application shell, local settings, first-run setup, and diagnostics. Cloud connections, virtual drive mounting, game scanning and launching, cache execution, and predictive prefetching are not implemented yet.

## What works today

- Desktop window with Dashboard, Cloud Drives, Game Library, Prepare, Smart Cache, Performance, Settings, and Diagnostics sections.
- First-run preferences for cache location and budget, free-space reserve, network estimate, and theme.
- Validated settings saved as versioned JSON under the current user's local application data directory.
- Settings loading and saving handled by a background worker so persistence I/O does not run in the immediate-mode UI update.
- Diagnostics for application paths, settings load state, and recent log entries.
- Structured Rust modules that separate UI, application services, domain types, persistence, and errors.

See [Features and status](docs/FEATURES.md) for the implemented behavior and current limitations.

## Build and run

Cerberus currently targets **Windows 10/11, x86-64, MSVC**.

1. Install Rust using [rustup](https://rustup.rs/) and install the Visual Studio C++ Build Tools with the Windows SDK.
2. Clone this repository and open PowerShell in its root directory.
3. Run `./build.ps1` to produce an optimized release build, or run `cargo run --target x86_64-pc-windows-msvc` for development.

The release executable is created at `target/x86_64-pc-windows-msvc/release/cerberus.exe`. Build output is local and is excluded from Git.

Additional instructions: [Building](docs/BUILDING.md) · [Development](docs/DEVELOPMENT.md) · [Troubleshooting](docs/TROUBLESHOOTING.md)

## Architecture

The Rust code is split into five main layers:

| Layer | Location | Responsibility |
| --- | --- | --- |
| UI | `src/ui/` | Window, navigation, and egui views |
| Application | `src/app/` | Coordinates operations, state, and background settings commands |
| Domain | `src/domain/` | Settings, IDs, typed units, and status models |
| Persistence | `src/persistence/` | User data paths and versioned settings storage |
| Errors | `src/error.rs` | Shared application error types |

Read [Architecture](docs/ARCHITECTURE.md) for the startup, settings, and persistence flows.

## Repository layout

```text
cerberus/
├── docs/                 # Feature status, architecture, build and development guides
├── src/
│   ├── app/              # Application service and shared state
│   ├── domain/           # UI-independent models and typed values
│   ├── persistence/      # Settings storage and application paths
│   └── ui/               # Window and view components
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
└── build.ps1
```

## Data locations

Settings, logs, and the default cache directory are stored below the current user's local application data directory in `Cerberus/`. For example, Windows resolves this beneath `%LOCALAPPDATA%`; no developer-specific absolute path is required by the project.

## Project relationship and license

Cerberus is an independent Rust project inspired by the general idea of cloud-hosted game storage. It is not affiliated with or endorsed by other software projects. Cerberus is licensed under the MIT License; see [LICENSE](LICENSE).
