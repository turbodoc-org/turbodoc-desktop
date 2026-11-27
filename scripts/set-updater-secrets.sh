#!/usr/bin/env bash
# Makes the key pair that signs updates, and stores its private half in the repository's
# "release" environment, which only v* tags can use. Run it yourself, once, in your own terminal:
#
#   scripts/set-updater-secrets.sh
#
# Every running copy of Turbodoc checks each update against the public key built into it
# (`PUBKEY` in src/updater.rs). Lose the private key and no installed copy can take another
# update: keep it, and its password, in your password manager. Make a new pair only to replace
# a leaked one, knowing that every copy then has to be reinstalled by hand.
#
# The key is written to a temporary folder and stored with `gh secret set` through pipes; the
# password goes through the environment, never a command line. The folder is removed at the
# end, after you've saved the key elsewhere. Nothing is printed but the public key.
set -euo pipefail

REPO=${REPO:-turbodoc-org/turbodoc-desktop}
ENVIRONMENT=release

die() { echo "error: $*" >&2; exit 1; }

command -v cargo-packager >/dev/null || die "cargo-packager isn't installed: cargo install cargo-packager --version 0.11.8 --locked"
command -v gh >/dev/null || die "the GitHub CLI (gh) isn't installed"
gh auth status >/dev/null 2>&1 || die "gh isn't signed in: run \`gh auth login\`"
gh api "repos/$REPO/environments/$ENVIRONMENT" >/dev/null 2>&1 || die "$REPO has no \"$ENVIRONMENT\" environment"
if gh secret list --repo "$REPO" --env "$ENVIRONMENT" --json name --jq '.[].name' | grep -qx CARGO_PACKAGER_SIGN_PRIVATE_KEY; then
  echo "$REPO already has an updater key. Replacing it means every installed copy must be reinstalled by hand."
  read -r -p "Replace it anyway? [y/N] " yes
  [[ $yes == [yY] ]] || die "nothing changed"
fi

DIR=$(mktemp -d)
trap 'rm -rf "$DIR"' EXIT
KEY="$DIR/turbodoc-updater.key"

# `signer generate` asks for the key's password itself, then writes the private key to $KEY
# and the public key to $KEY.pub.
echo "cargo packager asks for a password for the new key (twice):"
cargo packager signer generate --path "$KEY"
[[ -s $KEY && -s $KEY.pub ]] || die "cargo packager didn't write the key pair"

# The password, for the secret: checked by signing a file with it, as the release workflow does.
read -r -s -p "That password once more, to store it: " PASSWORD
echo
printf 'check' > "$DIR/check"
CARGO_PACKAGER_SIGN_PRIVATE_KEY=$(cat "$KEY") CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD=$PASSWORD \
  cargo packager signer sign "$DIR/check" >/dev/null 2>&1 || die "that password doesn't open the new key"
[[ -s $DIR/check.sig ]] || die "signing a test file made no .sig"

set_secret() { gh secret set "$1" --repo "$REPO" --env "$ENVIRONMENT" >/dev/null; }
set_secret CARGO_PACKAGER_SIGN_PRIVATE_KEY < "$KEY"
printf '%s' "$PASSWORD" | set_secret CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD
unset PASSWORD
echo "Stored: CARGO_PACKAGER_SIGN_PRIVATE_KEY, CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD"

echo
echo "Before going on, save the private key in your password manager. It's at:"
echo "  $KEY"
read -r -p "Saved it? [y/N] " yes
[[ $yes == [yY] ]] || echo "warning: removing it anyway; the copy in GitHub can't be read back out"

cat <<DONE

The public key, for PUBKEY in src/updater.rs:

$(cat "$KEY.pub")

Next:
  - Put it in src/updater.rs, commit, and release. That release is the first one that updates
    itself; copies older than it have to be updated by hand, once.
DONE
