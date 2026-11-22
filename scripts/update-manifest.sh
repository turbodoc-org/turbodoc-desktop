#!/usr/bin/env bash
# Makes this OS's update for running copies of Turbodoc, in dist/: the file the updater installs,
# its signature, and the manifest the app reads from the latest release (src/updater.rs):
#   macOS    Turbodoc_<version>_universal.app.tar.gz, for update-macos-aarch64.json and
#            update-macos-x86_64.json (one universal app serves both)
#   Windows  the setup .exe, for update-windows-x86_64.json
#   Linux    the AppImage, for update-linux-x86_64.json (the .deb is apt's to update)
#
# The release workflow runs it on a v* tag, after every other signature: the update's signature
# covers the file's final bytes, so signing or stapling after it would break it.
#
# Needs CARGO_PACKAGER_SIGN_PRIVATE_KEY and CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD
# (scripts/set-updater-secrets.sh), and GITHUB_REPOSITORY and GITHUB_REF_NAME, which Actions sets.
set -euo pipefail
cd "$(dirname "$0")/.."

die() { echo "error: $*" >&2; exit 1; }
: "${CARGO_PACKAGER_SIGN_PRIVATE_KEY:?the updater key is not set}"
: "${CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD:?the password of the updater key is not set}"
: "${GITHUB_REPOSITORY:?}"
TAG=${GITHUB_REF_NAME:?}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[[ $TAG == "v$VERSION" ]] || die "the tag $TAG doesn't match version $VERSION in Cargo.toml"

case "$(uname -s)" in
  Darwin)
    # An app that isn't notarized would replace a notarized one; don't offer it.
    if [[ -z ${APPLE_API_KEY_PATH:-} ]]; then
      echo "::warning::macOS isn't notarized in this release, so running copies aren't offered it."
      exit 0
    fi
    FILE="Turbodoc_${VERSION}_universal.app.tar.gz"
    # COPYFILE_DISABLE: no AppleDouble ._ files in the archive, which would break the signature.
    COPYFILE_DISABLE=1 tar -czf "dist/$FILE" -C dist Turbodoc.app
    FORMAT=app
    PLATFORMS="macos-aarch64 macos-x86_64"
    ;;
  MINGW* | MSYS* | CYGWIN*)
    FILE=$(cd dist && ls -- *-setup.exe | head -1)
    FORMAT=nsis
    PLATFORMS="windows-x86_64"
    ;;
  Linux)
    FILE=$(cd dist && ls -- *.AppImage | head -1)
    FORMAT=appimage
    PLATFORMS="linux-x86_64"
    ;;
  *) die "unsupported OS: $(uname -s)" ;;
esac
[[ -n $FILE && -f dist/$FILE ]] || die "no update file in dist/"

# Reads the key and its password from the environment, and writes dist/$FILE.sig.
cargo packager signer sign "dist/$FILE" >/dev/null
[[ -s dist/$FILE.sig ]] || die "signing made no dist/$FILE.sig"
SIGNATURE=$(tr -d '\r\n' < "dist/$FILE.sig")

# The release commit's subject, less "Release x.y.z: ", is the button's tooltip.
json() { sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'; }
NOTES=$(git log -1 --format=%s | sed "s/^Release [^:]*: *//" | json)
URL="https://github.com/$GITHUB_REPOSITORY/releases/download/$TAG/$FILE"
DATE=$(date -u +%Y-%m-%dT%H:%M:%SZ)
for platform in $PLATFORMS; do
  printf '{"version":"%s","notes":"%s","pub_date":"%s","url":"%s","signature":"%s","format":"%s"}\n' \
    "$VERSION" "$NOTES" "$DATE" "$URL" "$SIGNATURE" "$FORMAT" > "dist/update-$platform.json"
  echo "dist/update-$platform.json → $FILE"
done
