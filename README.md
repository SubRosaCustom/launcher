# SRCLauncher

[![ci](https://github.com/SubRosaCustom/launcher/actions/workflows/ci.yml/badge.svg)](https://github.com/SubRosaCustom/launcher/actions/workflows/ci.yml)
[![release](https://github.com/SubRosaCustom/launcher/actions/workflows/release.yml/badge.svg)](https://github.com/SubRosaCustom/launcher/actions/workflows/release.yml)

Downloads the latest release for your platfrom from SubRosaCustom/client_releases GitHub repo and loads it into the game

The launcher binary can also self-update from `SubRosaCustom/launcher` GitHub releases when the updater public key is baked in at build time through `SRC_LAUNCHER_UPDATER_PUBKEY`.

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
