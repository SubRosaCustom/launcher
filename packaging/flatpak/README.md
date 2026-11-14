# Flatpak packaging

Built and tested locally with `flatpak-builder`. Not submitted to Flathub; there is
no Flathub manifest repo for it yet.

## Files

- `com.src.launcher.yml` — manifest, GNOME 49 runtime plus the rust and node SDK extensions
- `com.src.launcher.desktop` — desktop entry, named after the app id as flatpak expects
- `com.src.launcher.metainfo.xml` — AppStream data, needed for a Flathub submission

## Building

```sh
flatpak install --system org.gnome.Sdk//49 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08 \
  org.freedesktop.Sdk.Extension.node22//25.08
sudo pacman -S flatpak-builder        # or org.flatpak.Builder from flathub

cd packaging/flatpak
flatpak-builder --user --install --force-clean build-dir com.src.launcher.yml
flatpak run com.src.launcher
```

Two build-options in the manifest exist for local builds only and are easy to trip
over:

- `build-args: --share=network`. The build sandbox has its own loopback, so a host
  running systemd-resolved on `127.0.0.53` gives npm and cargo `EAI_AGAIN` on every
  request. Without a working resolver npm also fails confusingly with
  `Exit handler never called!` instead of a network error.
- `HOME: /run/build/srclauncher`. npm needs a writable home, and the build sandbox
  `HOME` is not one.

Both get stripped or ignored once flathub builds with `--sandbox`, which is why a
flathub submission needs vendored dependencies.

The manifest builds from the release tag, not from the working tree. To build local
changes before tagging, copy it and swap the source:

```yaml
    sources:
      - type: dir
        path: ../..
        skip:
          - node_modules
          - dist
          - src-tauri/target
          - .git
```

## Sandbox notes

Two things need permissions that are not default:

- `--filesystem=home` plus `/run/media`, `/media`, `/mnt`. Steam libraries end up
  wherever the user put them, and the launcher reads the install to sync assets.
- `--talk-name=org.freedesktop.Flatpak`. The game cannot be started from inside the
  sandbox: it needs the host loader and glibc rather than the runtime's. The linux launch
  path detects `/.flatpak-info` and goes through
  `flatpak-spawn --host --directory=<game dir> --env=LD_PRELOAD=<client lib>`, so the
  game process is a normal host process with the client injected the same way as on a
  native install.

Because the client then runs on the host, `client_config_dir()` ignores the sandbox's
`XDG_CONFIG_HOME` when running in flatpak and uses `$HOME/.config/Sub Rosa Custom`
instead. Otherwise the launcher would write config into `~/.var/app/...` and the client
would read a different directory.

`WEBKIT_DISABLE_DMABUF_RENDERER=1` is set because webkitgtk's dmabuf renderer is a
common source of blank windows and startup aborts under flatpak on mesa and nvidia.

## Updater

The manifest flips `createUpdaterArtifacts` off the same way the AUR package does.
Flatpak updates come from the flatpak repository, and a binary that tries to rewrite
itself inside `/app` cannot work anyway. Client and asset updates still go through the
launcher.

## Before a Flathub submission

- screenshots in the metainfo, and a `developer_name`
- vendored dependencies (`cargo-sources.json` and a flatpak-npm-generator output), since
  flathub builds with network disabled
- either accept the `home` filesystem permission with a justification or narrow it to
  `~/.steam`, `~/.local/share/Steam` and the standard library mount points
