# SpriteCraft 0.3.0
2D pixel art & animation studio in Rust (egui). Adobe-style layout: title bar with menus, options bar, toolbox, project tabs, canvas with rulers, Preview/Color/Swatches/Layers panels, and a bottom Timeline.

## Install
    ./install.sh              # user install to ~/.local (installs deps + Rust automatically)
    ./install.sh --system     # /usr/local
    ./install.sh --uninstall

Re-running the script updates an existing install. Supports dnf, apt, pacman, zypper and apk.

## Build installable packages (written to ./dist)
    ./install.sh --package deb        # Debian/Ubuntu
    ./install.sh --package rpm        # Fedora/RHEL/openSUSE
    ./install.sh --package appimage   # portable, any distro
    ./install.sh --package flatpak    # built inside the Flatpak SDK (needs internet + flathub)
    ./install.sh --package exe        # Windows x64 (cross-compiled with mingw-w64 on Linux)
    ./install.sh --package msi        # Windows installer (built with wixl from msitools)
    ./install.sh --package dmg        # macOS - run on a Mac
    ./install.sh --package all        # everything this machine supports
    ./install.sh --list

Combine formats with commas (`--package deb,rpm,appimage`). Add `--no-deps` to skip system package installs.

Projects save as `.spc` (gzip JSON) in `~/SpriteCraft`. Help > Keyboard Shortcuts lists all shortcuts.
`tools/gen_icons.py` regenerates the PNG/ICO icons.

## Highlights
Start page (presets, recent files, open/import), project tabs, lasso/marquee/wand selections, RotSprite rotation,
outline / drop-shadow / auto-shade effects, tolerant color replace, "Project colors" panel (select pixels by color),
frame names and animation tags, crash recovery (autosave every 30 s), drag & drop of .spc/.png, light and dark themes,
command palette (Ctrl+K), and game-engine exports (Godot 4 SpriteFrames, Aseprite-style JSON for Phaser/PixiJS/Defold,
horizontal strips, CSS/HTML snippet).

## Releases
Pushing a tag builds and publishes every package automatically (`.github/workflows/release.yml`):

    git tag v0.3.0 && git push origin v0.3.0

The tag must match `version` in `Cargo.toml`. Artifacts: deb, rpm, AppImage, Flatpak, Windows exe + msi, macOS dmg (Intel and Apple Silicon) and `SHA256SUMS`.
`.github/workflows/ci.yml` runs the tests on every push and pull request.

Settings live in `~/.config/spritecraft/`, autosave recovery data in `~/.cache/spritecraft/recovery/`.
