# Troubleshooting

## Rust or Cargo is not found

Install Rust with rustup, restart PowerShell so its PATH is refreshed, then check `rustc --version` and `cargo --version`. The build script checks PATH first and falls back to rustup's standard per-user Cargo directory.

## `dlltool` or GNU import-library errors on Windows

Cerberus targets `x86_64-pc-windows-msvc`. Install the MSVC C++ Build Tools and Windows SDK, then build with the pinned target. Remove conflicting MinGW directories from the front of PATH if they cause Cargo to select GNU tools for the MSVC build.

## Settings do not load or save

Open Diagnostics and check the settings path and settings-load status. Logs are written beneath the user-local `Cerberus/logs/` directory. If a settings file is corrupt or uses a schema version newer than the application supports, Diagnostics reports the outcome; unsupported future-version files are preserved.

## App does not launch or appears unresponsive

Run from PowerShell to see startup errors, then check the Cerberus log. Confirm that the executable was built for the current Windows architecture and that the file is not still running from a previous build when rebuilding.
