#!/usr/bin/env bash
# SpriteCraft - build, install and package script.
#
# INSTALL
#   ./install.sh                  build and install for the current user (~/.local)
#   ./install.sh --system         install system-wide (/usr/local, uses sudo)
#   ./install.sh --uninstall      remove an existing installation
#
# PACKAGE  (artifacts are written to ./dist)
#   ./install.sh --package deb         Debian/Ubuntu package
#   ./install.sh --package rpm         Fedora/RHEL/openSUSE package
#   ./install.sh --package appimage    portable AppImage
#   ./install.sh --package flatpak     Flatpak bundle (built inside the Flatpak SDK)
#   ./install.sh --package exe         Windows x64 executable (cross-compiled on Linux, native on Windows)
#   ./install.sh --package msi         Windows installer (cross-built on Linux with wixl)
#   ./install.sh --package dmg         macOS disk image (must be run on a Mac)
#   ./install.sh --package all         everything this machine can build
#   Several can be combined:  --package deb,rpm,appimage
#
# OPTIONS
#   --no-deps     skip installing system packages
#   --list        list package formats and what this host supports
#   -h, --help    show this help
set -euo pipefail

APP=spritecraft
NAME=SpriteCraft
APP_ID=io.github.spritecraft.SpriteCraft
SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SRC_DIR"
VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
DIST="$SRC_DIR/dist"
FLATPAK_RUNTIME_VERSION="${FLATPAK_RUNTIME_VERSION:-24.08}"
WIN_TARGET=x86_64-pc-windows-gnu

SYSTEM=0; DEPS=1; UNINSTALL=0; LIST=0; PACKAGES=()
while [ $# -gt 0 ]; do
  case "$1" in
    --system) SYSTEM=1 ;;
    --no-deps) DEPS=0 ;;
    --uninstall) UNINSTALL=1 ;;
    --list) LIST=1 ;;
    --package|--packages) shift; [ $# -gt 0 ] || { echo "--package needs a value" >&2; exit 1; }
      IFS=',' read -ra _p <<< "$1"; PACKAGES+=("${_p[@]}") ;;
    --package=*) IFS=',' read -ra _p <<< "${1#*=}"; PACKAGES+=("${_p[@]}") ;;
    -h|--help) sed -n '2,28p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Unknown option: $1 (try --help)" >&2; exit 1 ;;
  esac
  shift
done

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!!\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31mxx\033[0m %s\n' "$*" >&2; exit 1; }

case "$(uname -s)" in
  Linux) HOST=linux ;;
  Darwin) HOST=macos ;;
  MINGW*|MSYS*|CYGWIN*) HOST=windows ;;
  *) HOST=unknown ;;
esac
SUDO=""; if [ "$(id -u)" -ne 0 ] && command -v sudo >/dev/null 2>&1; then SUDO="sudo"; fi

PM=""
for m in dnf apt-get pacman zypper apk brew; do
  if command -v "$m" >/dev/null 2>&1; then PM="$m"; break; fi
done

case "$(uname -m)" in
  x86_64|amd64) ARCH=x86_64; DEB_ARCH=amd64 ;;
  aarch64|arm64) ARCH=aarch64; DEB_ARCH=arm64 ;;
  *) ARCH="$(uname -m)"; DEB_ARCH="$(uname -m)" ;;
esac

supported_formats() {
  case "$HOST" in
    linux) echo "deb rpm appimage flatpak exe msi" ;;
    macos) echo "dmg" ;;
    windows) echo "exe" ;;
    *) echo "" ;;
  esac
}

if [ "$LIST" = 1 ]; then
  echo "SpriteCraft $VERSION - package formats"
  echo "  deb appimage rpm flatpak exe msi dmg"
  echo "Supported on this host ($HOST): $(supported_formats)"
  exit 0
fi

if [ "$SYSTEM" = 1 ]; then PREFIX=/usr/local; else PREFIX="$HOME/.local"; fi
BIN="$PREFIX/bin"; APPS="$PREFIX/share/applications"; ICONS="$PREFIX/share/icons/hicolor/scalable/apps"
as_target() { if [ "$SYSTEM" = 1 ]; then $SUDO "$@"; else "$@"; fi; }

# ---------------------------------------------------------------- uninstall
if [ "$UNINSTALL" = 1 ]; then
  log "Removing $NAME from $PREFIX"
  as_target rm -f "$BIN/$APP" "$APPS/$APP.desktop" "$ICONS/$APP.svg"
  log "Done. (Projects in ~/SpriteCraft were left untouched.)"
  exit 0
fi

# ---------------------------------------------------------------- dependencies
# pkgs <dnf> <apt> <pacman> <zypper> : install the given package lists with the detected package manager
pkgs() {
  [ "$DEPS" = 1 ] || return 0
  case "$PM" in
    dnf)     [ -z "$1" ] || $SUDO dnf install -y $1 ;;
    apt-get) [ -z "$2" ] || { $SUDO apt-get update; $SUDO apt-get install -y $2; } ;;
    pacman)  [ -z "$3" ] || $SUDO pacman -S --needed --noconfirm $3 ;;
    zypper)  [ -z "$4" ] || $SUDO zypper --non-interactive install $4 ;;
    *) warn "No supported package manager found - install the needed tools manually." ;;
  esac
}

install_build_deps() {
  [ "$DEPS" = 1 ] || return 0
  case "$HOST" in
    linux)
      log "Installing build dependencies ($PM)"
      case "$PM" in
        dnf) pkgs "gcc gcc-c++ make curl pkgconf-pkg-config libxkbcommon-devel libxkbcommon-x11 wayland-devel libX11-devel libxcb-devel libXcursor-devel libXrandr-devel libXi-devel mesa-libGL-devel mesa-libEGL-devel mesa-dri-drivers" "" "" "" ;;
        apt-get) pkgs "" "build-essential curl pkg-config libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libx11-dev libxcb1-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev libegl1-mesa-dev libgl1-mesa-dri" "" "" ;;
        pacman) pkgs "" "" "base-devel curl pkgconf libxkbcommon libxkbcommon-x11 wayland libx11 libxcb libxcursor libxrandr libxi mesa" "" ;;
        zypper) pkgs "" "" "" "gcc gcc-c++ make curl pkg-config libxkbcommon-devel libxkbcommon-x11-0 wayland-devel libX11-devel libxcb-devel libXcursor-devel libXrandr-devel libXi-devel Mesa-libGL-devel Mesa-libEGL-devel" ;;
        apk) $SUDO apk add build-base curl pkgconf libxkbcommon-dev wayland-dev libx11-dev libxcb-dev libxcursor-dev libxrandr-dev libxi-dev mesa-dev mesa-dri-gallium ;;
        *) die "Unsupported package manager. Install a C toolchain, pkg-config, curl and X11/Wayland/xkbcommon/Mesa GL dev libraries, then re-run with --no-deps." ;;
      esac ;;
    macos) command -v cc >/dev/null 2>&1 || { log "Installing Xcode command line tools"; xcode-select --install || true; } ;;
  esac
}

rust_ok() {
  command -v cargo >/dev/null 2>&1 || return 1
  local v; v=$(rustc --version 2>/dev/null | sed -E 's/rustc ([0-9]+)\.([0-9]+).*/\1 \2/') || return 1
  set -- $v
  [ "$1" -gt 1 ] || [ "$2" -ge 85 ]
}

ensure_rust() {
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  if ! rust_ok; then
    log "Installing/updating the Rust toolchain with rustup"
    if command -v rustup >/dev/null 2>&1; then
      rustup update stable
    elif [ "$HOST" = windows ] && command -v winget >/dev/null 2>&1; then
      winget install -e --id Rustlang.Rustup --accept-source-agreements --accept-package-agreements
      export PATH="$PATH:$HOME/.cargo/bin"
    else
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    fi
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  fi
  command -v cargo >/dev/null 2>&1 || die "cargo not found. Install Rust from https://rustup.rs and re-run."
}

# rustup is required for cross-compilation targets (distro-packaged cargo/rustc cannot add targets)
ensure_rustup() {
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  if ! command -v rustup >/dev/null 2>&1; then
    log "Installing rustup (needed for cross-compilation targets)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  command -v rustup >/dev/null 2>&1 || die "rustup not found. Install it from https://rustup.rs and re-run."
}

NATIVE_BUILT=0
build_native() {
  [ "$NATIVE_BUILT" = 1 ] && return 0
  ensure_rust
  log "Building $NAME $VERSION (release) - this can take a few minutes the first time"
  cargo build --release
  NATIVE_BUILT=1
}
NATIVE_BIN() { echo "$SRC_DIR/target/release/$APP"; }

desktop_file() { # $1 = Exec value, $2 = Icon value
  cat <<DESK
[Desktop Entry]
Type=Application
Name=$NAME
GenericName=Pixel Art & Animation Studio
Comment=2D pixel art and sprite animation editor
Exec=$1
Icon=$2
Terminal=false
Categories=Graphics;2DGraphics;RasterGraphics;
Keywords=pixel;sprite;animation;gif;art;
StartupWMClass=spritecraft
DESK
}

# ---------------------------------------------------------------- packagers
package_deb() {
  [ "$HOST" = linux ] || die "deb packages are built on Linux."
  pkgs "dpkg" "dpkg-dev" "dpkg" "dpkg"
  command -v dpkg-deb >/dev/null 2>&1 || die "dpkg-deb not found."
  build_native
  local stage; stage="$(mktemp -d)/pkg"
  install -Dm755 "$(NATIVE_BIN)" "$stage/usr/bin/$APP"
  install -Dm644 assets/$APP.svg "$stage/usr/share/icons/hicolor/scalable/apps/$APP.svg"
  install -Dm644 assets/$APP.png "$stage/usr/share/icons/hicolor/512x512/apps/$APP.png"
  mkdir -p "$stage/usr/share/applications" "$stage/DEBIAN"
  desktop_file "$APP" "$APP" > "$stage/usr/share/applications/$APP.desktop"
  cat > "$stage/DEBIAN/control" <<CTL
Package: $APP
Version: $VERSION
Section: graphics
Priority: optional
Architecture: $DEB_ARCH
Depends: libc6, libgl1, libegl1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libx11-6
Installed-Size: $(du -sk "$stage/usr" | cut -f1)
Maintainer: SpriteCraft Maintainers <spritecraft@localhost>
Description: 2D pixel art and animation studio
 SpriteCraft is a pixel art editor with layers, frames, onion skinning,
 palettes, filters, text, project tabs and PNG/sprite sheet/GIF export.
CTL
  mkdir -p "$DIST"
  dpkg-deb --root-owner-group --build "$stage" "$DIST/${APP}_${VERSION}_${DEB_ARCH}.deb" >/dev/null
  log "Built $DIST/${APP}_${VERSION}_${DEB_ARCH}.deb"
}

package_rpm() {
  [ "$HOST" = linux ] || die "rpm packages are built on Linux."
  pkgs "rpm-build" "rpm" "rpm-tools" "rpm-build"
  command -v rpmbuild >/dev/null 2>&1 || die "rpmbuild not found."
  build_native
  local top stage; top="$(mktemp -d)"; stage="$top/stage"
  install -Dm755 "$(NATIVE_BIN)" "$stage/usr/bin/$APP"
  install -Dm644 assets/$APP.svg "$stage/usr/share/icons/hicolor/scalable/apps/$APP.svg"
  install -Dm644 assets/$APP.png "$stage/usr/share/icons/hicolor/512x512/apps/$APP.png"
  mkdir -p "$stage/usr/share/applications" "$top"/{BUILD,RPMS,SPECS,SRPMS,BUILDROOT}
  desktop_file "$APP" "$APP" > "$stage/usr/share/applications/$APP.desktop"
  cat > "$top/SPECS/$APP.spec" <<SPEC
%global debug_package %{nil}
%global __strip /bin/true
Name:           $APP
Version:        $VERSION
Release:        1
Summary:        2D pixel art and animation studio
License:        MIT
BuildArch:      $ARCH
Requires:       libxkbcommon, libxkbcommon-x11, mesa-libGL, mesa-libEGL

%description
SpriteCraft is a pixel art editor with layers, frames, onion skinning,
palettes, filters, text, project tabs and PNG/sprite sheet/GIF export.

%install
mkdir -p %{buildroot}
cp -a $stage/. %{buildroot}/

%files
/usr/bin/$APP
/usr/share/applications/$APP.desktop
/usr/share/icons/hicolor/scalable/apps/$APP.svg
/usr/share/icons/hicolor/512x512/apps/$APP.png
SPEC
  rpmbuild -bb --define "_topdir $top" "$top/SPECS/$APP.spec" >"$top/rpm.log" 2>&1 || { cat "$top/rpm.log" >&2; return 1; }
  mkdir -p "$DIST"
  cp "$top"/RPMS/*/*.rpm "$DIST/"
  log "Built $(ls "$DIST"/${APP}-${VERSION}-*.rpm | head -1)"
}

package_appimage() {
  [ "$HOST" = linux ] || die "AppImages are built on Linux."
  command -v curl >/dev/null 2>&1 || pkgs "curl" "curl" "curl" "curl"
  build_native
  local top appdir tool; top="$(mktemp -d)"; appdir="$top/$NAME.AppDir"
  install -Dm755 "$(NATIVE_BIN)" "$appdir/usr/bin/$APP"
  install -Dm644 assets/$APP.png "$appdir/$APP.png"
  install -Dm644 assets/$APP.svg "$appdir/usr/share/icons/hicolor/scalable/apps/$APP.svg"
  ln -sf "$APP.png" "$appdir/.DirIcon"
  mkdir -p "$appdir/usr/share/applications"
  desktop_file "$APP" "$APP" > "$appdir/$APP.desktop"
  cp "$appdir/$APP.desktop" "$appdir/usr/share/applications/"
  cat > "$appdir/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/spritecraft" "$@"
RUN
  chmod +x "$appdir/AppRun"
  tool="${XDG_CACHE_HOME:-$HOME/.cache}/spritecraft/appimagetool-$ARCH.AppImage"
  if [ ! -x "$tool" ]; then
    log "Downloading appimagetool"
    mkdir -p "$(dirname "$tool")"
    curl -fL -o "$tool" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
    chmod +x "$tool"
  fi
  mkdir -p "$DIST"
  ARCH="$ARCH" "$tool" --appimage-extract-and-run "$appdir" "$DIST/$NAME-$VERSION-$ARCH.AppImage" >/dev/null
  log "Built $DIST/$NAME-$VERSION-$ARCH.AppImage"
}

package_flatpak() {
  [ "$HOST" = linux ] || die "Flatpaks are built on Linux."
  pkgs "flatpak flatpak-builder" "flatpak flatpak-builder" "flatpak flatpak-builder" "flatpak flatpak-builder"
  command -v flatpak-builder >/dev/null 2>&1 || die "flatpak-builder not found."
  local top; top="$(mktemp -d)"
  log "Preparing Flatpak build (runtime org.freedesktop.Platform//$FLATPAK_RUNTIME_VERSION)"
  flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
  desktop_file "$APP" "$APP_ID" > "$top/$APP_ID.desktop"
  cat > "$top/$APP_ID.yml" <<YML
app-id: $APP_ID
runtime: org.freedesktop.Platform
runtime-version: '$FLATPAK_RUNTIME_VERSION'
sdk: org.freedesktop.Sdk
sdk-extensions:
  - org.freedesktop.Sdk.Extension.rust-stable
command: $APP
finish-args:
  - --share=ipc
  - --socket=x11
  - --socket=wayland
  - --device=dri
  - --filesystem=home
build-options:
  append-path: /usr/lib/sdk/rust-stable/bin
  build-args:
    - --share=network
  env:
    CARGO_HOME: /run/build/$APP/cargo
modules:
  - name: $APP
    buildsystem: simple
    build-commands:
      - cargo build --release
      - install -Dm755 target/release/$APP /app/bin/$APP
      - install -Dm644 assets/$APP.svg /app/share/icons/hicolor/scalable/apps/$APP_ID.svg
      - install -Dm644 $APP_ID.desktop /app/share/applications/$APP_ID.desktop
    sources:
      - type: dir
        path: $SRC_DIR
        skip: [target, dist, .git]
      - type: file
        path: $top/$APP_ID.desktop
YML
  ( cd "$top" && flatpak-builder --user --force-clean --install-deps-from=flathub --repo="$top/repo" "$top/build" "$APP_ID.yml" )
  mkdir -p "$DIST"
  flatpak build-bundle "$top/repo" "$DIST/$NAME-$VERSION.flatpak" "$APP_ID"
  log "Built $DIST/$NAME-$VERSION.flatpak   (install: flatpak install --user $NAME-$VERSION.flatpak)"
}

build_windows_exe() { # leaves the exe at $WIN_EXE
  case "$HOST" in
    windows)
      ensure_rust
      log "Building Windows executable"
      cargo build --release
      WIN_EXE="$SRC_DIR/target/release/$APP.exe" ;;
    linux)
      pkgs "mingw64-gcc mingw64-binutils" "gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64" "mingw-w64-gcc" "mingw64-cross-gcc mingw64-cross-binutils"
      command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 || die "x86_64-w64-mingw32-gcc not found (install the mingw-w64 cross compiler)."
      ensure_rustup
      # Use rustup's own toolchain explicitly: a distro cargo earlier in PATH has no Windows std library.
      log "Installing rustup stable toolchain with the $WIN_TARGET target"
      rustup toolchain install stable --profile minimal --target "$WIN_TARGET"
      log "Cross-compiling for Windows ($WIN_TARGET)"
      CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc rustup run stable cargo build --release --target "$WIN_TARGET"
      WIN_EXE="$SRC_DIR/target/$WIN_TARGET/release/$APP.exe" ;;
    *) die "Windows builds need Linux (cross-compile) or Windows." ;;
  esac
  [ -f "$WIN_EXE" ] || die "Expected $WIN_EXE was not produced."
}

package_exe() {
  build_windows_exe
  mkdir -p "$DIST"
  cp "$WIN_EXE" "$DIST/$NAME-$VERSION-windows-x64.exe"
  log "Built $DIST/$NAME-$VERSION-windows-x64.exe"
}

package_msi() {
  [ "$HOST" = linux ] || die "MSI packages are cross-built on Linux (or WSL) with wixl from msitools."
  pkgs "msitools" "msitools wixl" "msitools" "msitools"
  command -v wixl >/dev/null 2>&1 || die "wixl not found (install msitools)."
  build_windows_exe
  local top; top="$(mktemp -d)"
  cp "$WIN_EXE" "$top/$NAME.exe"
  cp assets/$APP.ico "$top/$APP.ico"
  cat > "$top/$APP.wxs" <<WXS
<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="$NAME" Language="1033" Version="$VERSION" Manufacturer="$NAME" UpgradeCode="3F6B2C1E-8A47-4D6B-9C5E-2B7A1D0E4F93">
    <Package Description="$NAME $VERSION installer" Manufacturer="$NAME" InstallerVersion="500" Compressed="yes" Platform="x64"/>
    <MajorUpgrade DowngradeErrorMessage="A newer version of $NAME is already installed."/>
    <Media Id="1" Cabinet="$APP.cab" EmbedCab="yes"/>
    <Icon Id="AppIcon" SourceFile="$top/$APP.ico"/>
    <Property Id="ARPPRODUCTICON" Value="AppIcon"/>
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLDIR" Name="$NAME">
          <Component Id="MainExecutable" Guid="*" Win64="yes">
            <File Id="SpriteCraftExe" Name="$NAME.exe" Source="$top/$NAME.exe" KeyPath="yes"/>
          </Component>
        </Directory>
      </Directory>
      <Directory Id="ProgramMenuFolder">
        <Component Id="StartMenuShortcut" Guid="*" Win64="yes">
          <Shortcut Id="AppShortcut" Name="$NAME" Target="[INSTALLDIR]$NAME.exe" WorkingDirectory="INSTALLDIR" Icon="AppIcon"/>
          <RegistryValue Root="HKCU" Key="Software\\$NAME" Name="installed" Type="integer" Value="1" KeyPath="yes"/>
        </Component>
      </Directory>
    </Directory>
    <Feature Id="Complete" Level="1">
      <ComponentRef Id="MainExecutable"/>
      <ComponentRef Id="StartMenuShortcut"/>
    </Feature>
  </Product>
</Wix>
WXS
  mkdir -p "$DIST"
  wixl -v -a x64 -o "$DIST/$NAME-$VERSION-x64.msi" "$top/$APP.wxs"
  log "Built $DIST/$NAME-$VERSION-x64.msi"
}

package_dmg() {
  [ "$HOST" = macos ] || die "DMG images can only be built on macOS."
  build_native
  local top app; top="$(mktemp -d)"; app="$top/stage/$NAME.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp "$(NATIVE_BIN)" "$app/Contents/MacOS/$APP"
  local iconset="$top/$APP.iconset"; mkdir -p "$iconset"
  for s in 16 32 64 128 256 512; do
    sips -z $s $s assets/$APP.png --out "$iconset/icon_${s}x${s}.png" >/dev/null
    sips -z $((s*2)) $((s*2)) assets/$APP.png --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null 2>&1 || true
  done
  iconutil -c icns "$iconset" -o "$app/Contents/Resources/$APP.icns"
  cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleIdentifier</key><string>$APP_ID</string>
  <key>CFBundleExecutable</key><string>$APP</string>
  <key>CFBundleIconFile</key><string>$APP</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
  command -v codesign >/dev/null 2>&1 && codesign --force --deep -s - "$app" >/dev/null 2>&1 || true
  ln -s /Applications "$top/stage/Applications"
  mkdir -p "$DIST"
  rm -f "$DIST/$NAME-$VERSION-macos-$ARCH.dmg"
  hdiutil create -volname "$NAME" -srcfolder "$top/stage" -ov -format UDZO "$DIST/$NAME-$VERSION-macos-$ARCH.dmg" >/dev/null
  log "Built $DIST/$NAME-$VERSION-macos-$ARCH.dmg"
}

run_packages() {
  local want=() p
  for p in "${PACKAGES[@]}"; do
    case "$p" in
      all) for f in $(supported_formats); do want+=("$f"); done ;;
      deb|rpm|appimage|flatpak|exe|msi|dmg) want+=("$p") ;;
      *) die "Unknown package format '$p' (use --list)." ;;
    esac
  done
  install_build_deps
  local ok=() failed=()
  for p in "${want[@]}"; do
    log "Packaging: $p"
    if ( "package_$p" ); then ok+=("$p"); else failed+=("$p"); warn "Packaging '$p' failed."; fi
  done
  echo
  [ ${#ok[@]} -gt 0 ] && log "Done: ${ok[*]}   ->   $DIST"
  [ ${#failed[@]} -gt 0 ] && { warn "Failed: ${failed[*]}"; exit 1; }
  return 0
}

# ---------------------------------------------------------------- main
if [ ${#PACKAGES[@]} -gt 0 ]; then
  run_packages
  exit 0
fi

install_build_deps
build_native
log "Installing to $PREFIX"
as_target install -Dm755 "$(NATIVE_BIN)" "$BIN/$APP"
as_target install -Dm644 "assets/$APP.svg" "$ICONS/$APP.svg"
tmp="$(mktemp)"
desktop_file "$BIN/$APP" "$APP" > "$tmp"
as_target install -Dm644 "$tmp" "$APPS/$APP.desktop"; rm -f "$tmp"
command -v update-desktop-database >/dev/null 2>&1 && as_target update-desktop-database "$APPS" >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && as_target gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true

log "Installed: $BIN/$APP  (version $VERSION)"
case ":$PATH:" in *":$BIN:"*) ;; *) echo "Note: add $BIN to your PATH to launch with '$APP'." ;; esac
echo "Launch from your app menu or run: $APP"
