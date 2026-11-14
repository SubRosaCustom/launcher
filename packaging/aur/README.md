# AUR packaging

`srclauncher` is packaged on the AUR as a source build, not as a repack of the
release AppImage. That is deliberate: release artifacts have the Tauri
self-updater enabled, and a self-updating binary that pacman also owns is a mess.
The PKGBUILD flips `createUpdaterArtifacts` off, so a source build has no updater
pubkey baked in and `commands::get_launcher_update_state` reports the launcher as
not self-updatable. Updates come from `pacman -Syu`, which is what you want on
Arch. Client DLL/SO updates still come from `SubRosaCustom/client_releases` as
normal.

## Layout

- `PKGBUILD` is the canonical copy, kept in this repo
- `.SRCINFO` is never committed here, CI regenerates it inside an Arch container
- `.github/workflows/aur.yml` publishes on release (or manually via dispatch)

## First time setup (manual, once)

1. Create an account on https://aur.archlinux.org and add an SSH public key in
   *My Account*.
2. Push the package for the first time. The AUR git endpoint will not create a
   new pkgbase, so the initial submit has to happen once by hand:

   ```sh
   git clone ssh://aur@aur.archlinux.org/srclauncher.git
   cd srclauncher
   cp path/to/launcher/packaging/aur/PKGBUILD .
   makepkg --printsrcinfo > .SRCINFO
   git add PKGBUILD .SRCINFO
   git commit -m "initial import: srclauncher 1.0.0-1"
   git push origin master
   ```

   If the push is rejected because the pkgbase does not exist yet, submit it once
   through https://aur.archlinux.org/submit/ with the same PKGBUILD, then keep
   using the git push flow.

3. Add the deploy key as the repository secret `AUR_SSH_PRIVATE_KEY`. Generate a
   dedicated key (`ssh-keygen -t ed25519 -C "aur-ci" -f aur-ci -N ""`), register
   the public half on the AUR account, and paste the private half into the
   secret. Anything that lands in that AUR account can be pushed by CI, so do not
   reuse your personal key.

## Publishing after that

The `aur` workflow runs on `release: published`, skips prereleases, and can also
be started by hand:

```sh
gh workflow run aur.yml -f version=1.0.1
```

What it does:

1. reads the version from the release tag (`launcher-v1.0.0` -> `1.0.0`)
2. downloads the tag tarball and computes the sha256
3. copies `packaging/aur/PKGBUILD`, rewrites `pkgver`, `pkgrel` and `sha256sums`
4. regenerates `.SRCINFO` with `makepkg --printsrcinfo` in `archlinux:base-devel`
5. commits `upgpkg: srclauncher <ver>-<rel>` and pushes to the AUR

If the version matches what the AUR already has, the workflow bumps `pkgrel`
instead of resetting it, and does nothing when there is no diff.

## Local test build

```sh
cp packaging/aur/PKGBUILD /tmp/aur-test && cd /tmp/aur-test
makepkg -si --nocheck
```

Clean up afterwards with `pacman -Rns srclauncher`, and remember `makepkg` also
leaves `~/srclauncher-*.pkg.tar.zst`.

## Known gaps

- No separate `srclauncher-bin`: the release AppImage ships the updater, so a
  binary repack would try to overwrite its own pacman-owned files
- The package builds with `options=('!lto')`. makepkg appends `-flto=auto` to
  `CFLAGS`, and `ring` compiles C sources into the crate, so those objects come
  out as gcc LTO IR that the rustc/lld link stage silently drops. Every
  `ring_core_*` symbol then shows up undefined at the final link. Same reason a
  plain `-flto` build of any rust crate that uses `ring` fails.
- The binary is not reproducible: `npm ci` and `cargo` both fetch at build time.
  This is common for Tauri packages and fine for the AUR, but it means the
  build needs network access
- No `LICENSE` in the repo, so the PKGBUILD declares `license=('custom')` and
  installs `legal/RISK-NOTICE.md` under `/usr/share/licenses/srclauncher/`
