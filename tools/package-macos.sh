#!/usr/bin/env bash
# Package the standalone OctosCode desktop app (crates/octoscode-desktop) as a
# self-contained, ad-hoc-signed macOS bundle plus a zip to copy to another Mac
# (A33, decision D10f; docs/BUILD-macos.md).
#
#   tools/package-macos.sh [--out <dir>] [--no-build]
#
# Output (default <repo>/target/macos-app):
#   OctosCode.app                      the bundle
#   OctosCode-macos-<arch>.zip         the bundle, zipped with ditto (keeps the signature)
#
# Self-contained means nothing resolves through this machine's paths:
# - the binary is built with MAKEPAD=apple_bundle and MAKEPAD_PACKAGE_DIR=makepad, so
#   makepad reads every crate resource (`crate_resource("makepad_widgets:resources/…")`,
#   the module's own `self:resources/…`) from
#   OctosCode.app/Contents/Resources/makepad/<crate>/resources/…, and never from the
#   build machine's cargo checkouts; this script copies every `resources/` directory of
#   the app's dependency graph there (what cargo-makepad does for an iOS bundle);
# - the module's design/ tree and its own faces and icons ride inside the binary (the
#   module's build.rs embed) and are materialized under $HOME/.octoscode/design at the
#   first launch; a packaged build never reads the build checkout (design.rs
#   set_packaged), and file_resource(<that path>) loads in a packaged build
#   (patches/makepad/packaged-file-resource.patch).
#
# The binary uses its own cargo profile (`app-bundle`, release settings), so it never
# overwrites a normal `cargo build --release` and the bundle-only makepad cfg never
# leaks into one. Needs the forks tools/build-macos.sh prepares (.forks/).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
OUT=""
BUILD=1
while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --no-build) BUILD=0; shift ;;
    -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "package-macos: unknown argument $1" >&2; exit 2 ;;
  esac
done
[ "$(uname -s)" = "Darwin" ] || { echo "package-macos: macOS only" >&2; exit 1; }
for f in makepad-fork/widgets/Cargo.toml octosense-fork/apps/appcard/app/crates/octos-app-transport/Cargo.toml \
         octoscript-makepad-fork/crates/octoscript-makepad/Cargo.toml; do
  [ -f "$REPO/.forks/$f" ] || { echo "package-macos: .forks/$f is missing: run tools/build-macos.sh first" >&2; exit 1; }
done

TARGET_DIR="${CARGO_TARGET_DIR:-$REPO/target}"
OUT="${OUT:-$TARGET_DIR/macos-app}"
APP_NAME="OctosCode"
BIN_NAME="octoscode"
BUNDLE_ID="org.octos.octoscode"
ARCH="$(uname -m)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO/Cargo.toml" | head -1)"
BIN="$TARGET_DIR/app-bundle/$BIN_NAME"

if [ "$BUILD" = 1 ]; then
  echo "package-macos: cargo build --profile app-bundle -p octoscode-desktop (MAKEPAD=apple_bundle MAKEPAD_PACKAGE_DIR=makepad)"
  (cd "$REPO" && MAKEPAD=apple_bundle MAKEPAD_PACKAGE_DIR=makepad \
    cargo build --profile app-bundle -p octoscode-desktop --bin "$BIN_NAME")
fi
[ -x "$BIN" ] || { echo "package-macos: $BIN was not built" >&2; exit 1; }

APP="$OUT/$APP_NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/makepad"
cp "$BIN" "$APP/Contents/MacOS/$BIN_NAME"

# Every resources/ directory of the app's dependency graph (host platform), under the
# crate's library name: what makepad's packaged loader asks for
# (<Resources>/makepad/<crate>/resources/<file>).
(cd "$REPO" && cargo metadata --format-version 1 --filter-platform "$(rustc -vV | sed -n 's/^host: //p')") \
  | python3 "$HERE/package_resources.py" octoscode-desktop "$APP/Contents/Resources/makepad"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key><string>$BIN_NAME</string>
    <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
    <key>CFBundleName</key><string>$APP_NAME</string>
    <key>CFBundleDisplayName</key><string>$APP_NAME</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST
printf 'APPL????' > "$APP/Contents/PkgInfo"

# Ad-hoc signature: no identity, runs on the Mac it is copied to after the first
# right-click > Open (it is not notarized).
codesign --force --sign - --timestamp=none "$APP"
codesign --verify --strict "$APP"

ZIP="$OUT/$APP_NAME-macos-$ARCH.zip"
rm -f "$ZIP"
(cd "$OUT" && ditto -c -k --sequesterRsrc --keepParent "$APP_NAME.app" "$(basename "$ZIP")")

echo "package-macos: bundle  $APP ($(du -sh "$APP" | cut -f1); binary $(du -sh "$APP/Contents/MacOS/$BIN_NAME" | cut -f1), resources $(du -sh "$APP/Contents/Resources" | cut -f1))"
echo "package-macos: zip     $ZIP ($(du -sh "$ZIP" | cut -f1))"
echo "package-macos: run     open \"$APP\"    (or \"$APP/Contents/MacOS/$BIN_NAME\" --remote <port> for the instrument)"
