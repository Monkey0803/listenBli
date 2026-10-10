#!/usr/bin/env bash
#
# Build listenBli and package it as a double-clickable macOS app bundle.
#
#   scripts/bundle-macos.sh                  # native arch, release build
#   scripts/bundle-macos.sh --universal      # arm64 + x86_64 in one bundle
#   scripts/bundle-macos.sh --debug          # cargo debug profile (faster build)
#   ARCH=arm64 scripts/bundle-macos.sh       # force one arch (no --universal)
#
# The result is `dist/ListenBli.app`, ad-hoc signed so macOS treats it as a
# normal local application. It is not notarised: if the bundle is ever copied
# through a download (which sets the quarantine flag), clear it with
#   xattr -dr com.apple.quarantine /Applications/ListenBli.app
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# cargo usually lives in ~/.cargo/bin, which a non-login shell may not have.
if ! command -v cargo >/dev/null 2>&1; then
  if [ -x "$HOME/.cargo/bin/cargo" ]; then
    PATH="$HOME/.cargo/bin:$PATH"
  else
    echo "error: cargo not found; install Rust from https://rustup.rs" >&2
    exit 1
  fi
fi

PROFILE=release
CARGO_FLAGS=(--release)
UNIVERSAL=0
for arg in "$@"; do
  case "$arg" in
    --debug) PROFILE=debug; CARGO_FLAGS=() ;;
    --universal) UNIVERSAL=1 ;;
    -h|--help) sed -n '2,17p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "error: unknown argument: $arg" >&2; exit 2 ;;
  esac
done

APP_NAME="ListenBli"
BUNDLE_ID="${BUNDLE_ID:-com.listenbli.app}"
EXECUTABLE="listenBli"
MIN_MACOS="${MIN_MACOS:-11.0}"

# Cargo.toml is the single source of truth for the version and the project URL,
# so the About panel cannot drift from the crate.
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)"
VERSION="${VERSION:-0.0.0}"
REPOSITORY="$(sed -n 's/^repository *= *"\(.*\)"/\1/p' Cargo.toml | head -1)"

# macOS prints `CFBundleVersion` in parentheses in the About panel, so filling it
# with the version again would read "0.1.0 (0.1.0)". The commit count is the
# monotonic build number that key is meant to hold.
BUILD="$(git -C "$ROOT" rev-list --count HEAD 2>/dev/null || true)"
BUILD="${BUILD:-1}"

COPYRIGHT="MIT licensed"
if [ -n "$REPOSITORY" ]; then
  COPYRIGHT="$COPYRIGHT · $REPOSITORY"
fi

APP="dist/$APP_NAME.app"

# ---------------------------------------------------------------------------
# build — nothing is deleted until a binary exists, so a missing toolchain
# target leaves the previous bundle in place.
# ---------------------------------------------------------------------------
BINARY=""
if [ "$UNIVERSAL" = "1" ]; then
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    if ! rustup target list --installed | grep -qx "$target"; then
      echo "error: $target is not installed. Run:" >&2
      echo "  rustup target add aarch64-apple-darwin x86_64-apple-darwin" >&2
      exit 1
    fi
  done
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    echo "==> cargo build ${CARGO_FLAGS[*]} --target $target"
    cargo build "${CARGO_FLAGS[@]}" --target "$target"
  done
  mkdir -p dist
  echo "==> lipo"
  lipo -create \
    "target/aarch64-apple-darwin/$PROFILE/listenbli" \
    "target/x86_64-apple-darwin/$PROFILE/listenbli" \
    -output "dist/$EXECUTABLE-universal"
  BINARY="dist/$EXECUTABLE-universal"
else
  ARCH="${ARCH:-$(uname -m)}"
  case "$ARCH" in
    arm64) TARGET=aarch64-apple-darwin ;;
    x86_64) TARGET=x86_64-apple-darwin ;;
    *) echo "error: unsupported ARCH=$ARCH" >&2; exit 2 ;;
  esac
  echo "==> cargo build ${CARGO_FLAGS[*]} --target $TARGET"
  cargo build "${CARGO_FLAGS[@]}" --target "$TARGET"
  BINARY="target/$TARGET/$PROFILE/listenbli"
fi

# ---------------------------------------------------------------------------
# bundle
# ---------------------------------------------------------------------------
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/$EXECUTABLE"
strip -x "$APP/Contents/MacOS/$EXECUTABLE" 2>/dev/null || true

if [ "$UNIVERSAL" = "1" ]; then
  rm -f "dist/$EXECUTABLE-universal"
fi

if [ ! -f assets/ListenBli.icns ]; then
  echo "==> generating the icon"
  python3 scripts/make-icns.py
fi
cp "assets/ListenBli.icns" "$APP/Contents/Resources/ListenBli.icns"

sed -e "s|@EXECUTABLE@|$EXECUTABLE|g" \
    -e "s|@BUNDLE_ID@|$BUNDLE_ID|g" \
    -e "s|@VERSION@|$VERSION|g" \
    -e "s|@BUILD@|$BUILD|g" \
    -e "s|@COPYRIGHT@|$COPYRIGHT|g" \
    -e "s|@MIN_MACOS@|$MIN_MACOS|g" \
    packaging/macos/Info.plist.in > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null

printf 'APPL????' > "$APP/Contents/PkgInfo"

# ---------------------------------------------------------------------------
# signing
# ---------------------------------------------------------------------------
# Ad-hoc ("-") signing is enough for a locally built app and keeps the bundle
# self-consistent; replace with a Developer ID identity to distribute it.
IDENTITY="${CODESIGN_IDENTITY:--}"
echo "==> codesign ($IDENTITY)"
codesign --force --timestamp=none --sign "$IDENTITY" \
  --identifier "$BUNDLE_ID" "$APP"
codesign --verify --strict "$APP"

echo
echo "built $ROOT/$APP"
echo "  version : $VERSION (build $BUILD)"
echo "  repo    : ${REPOSITORY:-（未在 Cargo.toml 里写 repository）}"
echo "  arch    : $(lipo -archs "$APP/Contents/MacOS/$EXECUTABLE" 2>/dev/null || echo unknown)"
echo "  open it : open \"$ROOT/$APP\""
