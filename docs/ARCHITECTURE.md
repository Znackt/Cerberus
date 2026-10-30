# Architecture

Cerberus separates the immediate-mode UI from application coordination and persistence. This lets UI code submit work and display results without owning file-format or domain rules.

## Modules

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Initializes logging, creates `AppService`, configures the native window, and starts eframe. |
| `src/ui/` | Navigation, frame rendering, settings draft state, dashboard, and diagnostics views. |
| `src/app/` | `AppService`, shared `AppState`, settings command queue, and operation coordination. |
| `src/domain/` | Serializable settings, drive/game models, typed IDs, units, and status types. |
| `src/persistence/` | `AppPaths` and `SettingsStore`, including load status, validation, migration, and file replacement. |
| `src/error.rs` | Shared typed error and result definitions. |

## Startup and settings flow

1. `main` initializes a file logger under the user-local Cerberus data directory and falls back to stderr if the log file cannot be opened.
2. `AppService::new` resolves application paths, creates default in-memory state, and starts a worker thread.
3. The worker loads and validates settings, records the load status, then signals that settings are ready. UI views wait for this readiness state before treating defaults as first-run settings.
4. `MainWindow` creates the Dashboard, Settings, and Diagnostics views and routes sidebar navigation to the corresponding view.
5. Settings edits remain in a view-local draft until validation succeeds and the user saves. Save/load commands are sent to the worker; the UI receives completion through a one-shot result channel.
6. `SettingsStore` serializes settings to JSON and reports load, migration, recovery, and unsupported-schema outcomes to the service and Diagnostics view.

```mermaid
flowchart LR
    Main[main.rs] --> Service[AppService]
    Service --> Worker[Settings worker]
    Worker --> Store[SettingsStore]
    Store --> File[(settings.json)]
    Main --> Window[MainWindow]
    Window --> Views[Dashboard / Settings / Diagnostics]
    Views -->[commands and results] Service
    Service --> State[Shared AppState]
    State --> Views
```

## Persistence and data paths

By default, `AppPaths` uses the current user's local application data directory and creates `Cerberus/settings.json`, `Cerberus/cache/`, and `Cerberus/logs/` beneath it. Tests can inject a temporary data root. Settings include a schema version; future schema versions are rejected without overwriting the original file.

## UI boundary

Domain and persistence code do not depend on egui. UI code consumes service operations and domain values. Filesystem operations that can block the frame loop belong on the service worker or a dedicated background task, not directly in view drawing code.
