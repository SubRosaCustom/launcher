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

Arch Linux (AUR, source build):

```sh
paru -S srclauncher   # or yay -S srclauncher
```

The AUR package builds from the release tag and does not ship the Tauri self-updater, since pacman owns the installed files. Launcher updates come from `pacman -Syu` instead. The PKGBUILD lives in `packaging/aur/`, see `packaging/aur/README.md`.

Flatpak (local build, not on Flathub yet):

```sh
flatpak install --system org.gnome.Sdk//49 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08 \
  org.freedesktop.Sdk.Extension.node22//25.08
cd packaging/flatpak && flatpak-builder --user --install --force-clean build-dir com.src.launcher.yml
flatpak run com.src.launcher
```

The sandbox gets read/write access to `$HOME` and the usual removable-media mounts so it can find Steam libraries, and it starts the game on the host through `flatpak-spawn --host` because the sandbox runtime cannot load a Source engine binary. Like the AUR package it ships without the self-updater. See `packaging/flatpak/README.md`.

Notes:
- Linux `AppImage` may need `chmod +x srclauncher_<version>_amd64.AppImage` before first run
- `.deb` and `.rpm` releases are optional package formats; `AppImage` is the simplest Linux path
- Launcher updates come from `SubRosaCustom/launcher`
- Client DLL/SO updates come from `SubRosaCustom/client_releases`

Packaging releases:
- `packaging/flatpak/com.src.launcher.yml` pins the release tag and commit, bump both on every release
- `packaging/aur/PKGBUILD` is the canonical PKGBUILD
- `.github/workflows/aur.yml` pushes a new version to the AUR on publish, or on workflow dispatch with `version` input
- requires the `AUR_SSH_PRIVATE_KEY` secret and a key registered on the AUR account

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
