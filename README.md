# SRCLauncher

[![ci](https://github.com/SubRosaCustom/launcher/actions/workflows/ci.yml/badge.svg)](https://github.com/SubRosaCustom/launcher/actions/workflows/ci.yml)
[![release](https://github.com/SubRosaCustom/launcher/actions/workflows/release.yml/badge.svg)](https://github.com/SubRosaCustom/launcher/actions/workflows/release.yml)

Downloads the latest GitHub release for your platfrom from SubRosaCustom/client_releases and loads it into the game

The launcher binary can also self-update from `SubRosaCustom/launcher` GitHub releases when the updater public key is baked in at build time through `SRC_LAUNCHER_UPDATER_PUBKEY`.

## Installation

1. Open the latest release in `SubRosaCustom/launcher`.
2. Download the file for your platform.
3. Install or run it:
   Windows: use `srclauncher_<version>_x64-setup.exe` or `srclauncher_<version>_x64_en-US.msi`
   Linux: use `srclauncher_<version>_amd64.AppImage`
4. Launch SRCLauncher.
5. If Sub Rosa is not detected automatically, set the executable path in Settings.

Notes:
- Linux `AppImage` may need `chmod +x srclauncher_<version>_amd64.AppImage` before first run
- `.deb` and `.rpm` releases are optional package formats; `AppImage` is the simplest Linux path
- Launcher updates come from `SubRosaCustom/launcher`
- Client DLL/SO updates come from `SubRosaCustom/client_releases`

Launcher release requirements:
- publish releases in `SubRosaCustom/launcher`
- attach signed Tauri updater artifacts and `latest.json`
- provide `TAURI_SIGNING_PRIVATE_KEY` during release builds
- provide `SRC_LAUNCHER_UPDATER_PUBKEY` during launcher builds

## Development

- Install deps: `npm ci`
- Frontend build: `npm run build`
- Rust checks: `cargo check --manifest-path src-tauri/Cargo.toml`
- Rust tests: `cargo test --manifest-path src-tauri/Cargo.toml`
