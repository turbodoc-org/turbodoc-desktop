#!/usr/bin/env bash
# Builds Turbodoc for this OS and packages it with cargo-packager (Packager.toml), into dist/:
#   macOS    Turbodoc.app and Turbodoc_<version>_universal.dmg (Apple silicon and Intel in one)
#   Windows  Turbodoc_<version>_x64_en-US.msi and Turbodoc_<version>_x64-setup.exe
#   Linux    turbodoc_<version>_amd64.deb and Turbodoc_<version>_x86_64.AppImage
#
#   scripts/package.sh              # every format for this OS
#   scripts/package.sh --native     # macOS: only this Mac's architecture (a faster local build)
#   scripts/package.sh --build-only # then --package-only: what CI does, to sign the binary between
#
# Needs `cargo install cargo-packager --locked`. On macOS it signs with APPLE_SIGNING_IDENTITY
# (and notarizes with the variables Packager.toml lists), or ad hoc without one. Windows
# signing happens in the release workflow, around this script.
set -euo pipefail
cd "$(dirname "$0")/.."

NATIVE=0
STAGES=build,package
for arg in "$@"; do
  case "$arg" in
    --native) NATIVE=1 ;;
    --build-only) STAGES=build ;;
    --package-only) STAGES=package ;;
    *) echo "Unknown option $arg" >&2; exit 1 ;;
  esac
done
building() { [[ $STAGES == *build* ]]; }
# CI passes secrets that aren't set as empty variables; the packager would try to use them.
for v in APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID; do
  if [[ -z "${!v:-}" ]]; then unset "$v"; fi
done
BIN=target/dist
TARGET=

case "$(uname -s)" in
  Darwin)
    FORMATS=app,dmg
    if [[ $NATIVE == 0 ]]; then
      # Homebrew's Rust has no rustup; a universal build needs both standard libraries.
      if building && ! command -v rustup >/dev/null; then
        echo "A universal build needs rustup (https://rustup.rs). Use --native for this Mac only." >&2
        exit 1
      fi
      BIN=target/universal
      TARGET="--target universal-apple-darwin" # names the .dmg "universal"
      if building; then
        rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
        cargo build --locked --profile dist --target aarch64-apple-darwin
        cargo build --locked --profile dist --target x86_64-apple-darwin
        mkdir -p "$BIN"
        lipo -create -output "$BIN/turbodoc" target/aarch64-apple-darwin/dist/turbodoc target/x86_64-apple-darwin/dist/turbodoc
      fi
    elif building; then
      cargo build --locked --profile dist
    fi
    ;;
  MINGW* | MSYS* | CYGWIN*)
    FORMATS=wix,nsis
    if building; then cargo build --locked --profile dist; fi
    ;;
  Linux)
    FORMATS=deb,appimage
    if building; then cargo build --locked --profile dist; fi
    ;;
  *)
    echo "Unsupported OS: $(uname -s)" >&2
    exit 1
    ;;
esac

[[ $STAGES == *package* ]] || exit 0

# The packager reads the version, the binary's folder and the macOS signing identity only from
# its config, and resolves the config's paths beside it: package from a copy here that fills
# them in.
# Packager.toml signs ad hoc; APPLE_SIGNING_IDENTITY ("Developer ID Application: …") replaces it.
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
CONFIG=Packager.build.toml
trap 'rm -f Packager.build.toml' EXIT
{
  echo "version = \"$VERSION\""
  echo "binaries-dir = \"$BIN\""
  ID=${APPLE_SIGNING_IDENTITY:--} perl -pe 's/^signing-identity = .*/signing-identity = "$ENV{ID}"/' Packager.toml
} > "$CONFIG"
rm -rf dist
cargo packager -c "$CONFIG" -f "$FORMATS" $TARGET
echo "Packages are in dist/"
