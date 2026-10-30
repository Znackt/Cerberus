# Features and implementation status

This page describes the behavior present in the source today.

## Available in the current build

| Feature | Status | Behavior |
| --- | --- | --- |
| Desktop shell | Implemented | Resizable Windows window, sidebar navigation, and eight view sections. |
| Dashboard | Implemented | Shows the current setup state and links to Settings when setup is required. |
| First-run setup | Implemented | Edits a draft, validates values, saves settings, and reports persistence errors in the view. |
| Settings | Implemented | Cache root, cache budget, minimum free space, estimated network speed, and theme preference. |
| Settings persistence | Implemented | Versioned JSON, schema migration/rejection handling, validation, recovery reporting, and background-thread I/O. |
| Diagnostics | Implemented | Displays application paths, settings-load status, and dependency status for rclone/WinFsp. |
| Logging | Implemented | Writes application logs to the user-local Cerberus log directory, with stderr fallback. |
| Domain types | Implemented | Typed IDs, byte/network units, settings, and drive/game status models. |
| Cloud Drive Configuration | Implemented | Configure remotes with rclone, test connections asynchronously, and manage drive definitions. |
| Virtual Drive Mounting | Implemented | Mount/unmount using rclone and WinFsp with process supervision, auto-mount, and startup restoration. |
| Dependency Management | Implemented | Detects rclone and WinFsp installations, reports status in diagnostics. |

## Not implemented yet

- Game discovery, library editing, and launching.
- Cache population, eviction, and runtime performance monitoring.
- Predictive file access, prefetching, and background agents.
- Cloud-provider auth flows (beyond manual rclone config).

The settings model contains fields reserved for later phases. Their presence does not mean the corresponding runtime capability is active.
