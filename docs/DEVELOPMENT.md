# Development guide

## Source layout

```text
src/
├── app/          # Service coordination and shared state
├── domain/       # UI-independent settings, IDs, units, and statuses
├── persistence/  # App paths and settings serialization/migration
├── ui/           # Main window and view components
├── error.rs      # Shared error types
├── lib.rs        # Public library module exports
└── main.rs       # Desktop executable entry point
```

Keep domain rules independent of egui. Put settings validation and version handling in persistence/domain code, and route potentially blocking file operations through the application service's worker boundary.

## Local workflow

1. Make the smallest change in the layer that owns the behavior.
2. Add or update unit tests alongside the Rust module that owns the behavior.
3. Run formatting, tests, and Clippy using the commands in [Building](BUILDING.md).
4. Review `git diff` and `git status` before committing. Do not commit `target/`, local settings/logs, credentials, screenshots with private data, or machine-specific absolute paths.

The current tests cover settings validation/persistence, service operations, settings draft behavior, and diagnostics presentation. Update this guide as the suite grows.
