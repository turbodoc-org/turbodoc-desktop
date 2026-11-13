#!/usr/bin/env bash
# Stores the Apple signing and notarization secrets in the repository's "release" environment,
# which only v* tags can use. Run it yourself, in your own terminal:
#
#   scripts/set-apple-secrets.sh path/to/DeveloperID.p12 path/to/AuthKey_XXXXXXXXXX.p8
#
# It checks each piece before storing it: that the password opens the .p12, that the .p12 holds
# a "Developer ID Application" identity, and that Apple accepts the API key. Values go from the
# files and hidden prompts straight to `gh secret set` through pipes: nothing is printed, put on
# a command line or written to disk. GitHub encrypts each secret with the repository's public
# key before it leaves this Mac.
set -euo pipefail

REPO=${REPO:-turbodoc-org/turbodoc-desktop}
ENVIRONMENT=release

die() { echo "error: $*" >&2; exit 1; }
# Keychain Access on older macOS exports with RC2 and 3DES, which OpenSSL 3 and later read
# only with -legacy.
p12() { openssl pkcs12 "$@" 2>/dev/null || openssl pkcs12 -legacy "$@" 2>/dev/null; }

[[ $# == 2 ]] || die "usage: $0 <certificate.p12> <AuthKey_KEYID.p8>"
P12=$1
P8=$2
[[ -f $P12 ]] || die "no file at $P12"
[[ -f $P8 ]] || die "no file at $P8"
command -v gh >/dev/null || die "the GitHub CLI (gh) isn't installed"
gh auth status >/dev/null 2>&1 || die "gh isn't signed in: run \`gh auth login\`"
gh api "repos/$REPO/environments/$ENVIRONMENT" >/dev/null 2>&1 || die "$REPO has no \"$ENVIRONMENT\" environment"

# The certificate. The password stays in this process: `openssl` reads it from the environment
# and `gh` from a pipe, never from arguments other processes can see.
read -r -s -p "Password of $(basename "$P12"): " P12_PASSWORD
echo
export P12_PASSWORD
CERT=$(p12 -in "$P12" -clcerts -nokeys -passin env:P12_PASSWORD) || die "that password doesn't open $P12"
SUBJECT=$(printf '%s\n' "$CERT" | openssl x509 -noout -subject -nameopt multiline 2>/dev/null)
IDENTITY=$(printf '%s\n' "$SUBJECT" | sed -n 's/^ *commonName *= *//p' | head -1)
[[ $IDENTITY == "Developer ID Application:"* ]] || die "the certificate is \"$IDENTITY\", not a Developer ID Application certificate"
p12 -in "$P12" -nocerts -nodes -passin env:P12_PASSWORD | grep -q "PRIVATE KEY" || die "$P12 has no private key: export the identity (certificate and key), not the certificate alone"
EXPIRES=$(printf '%s\n' "$CERT" | openssl x509 -noout -enddate | cut -d= -f2)
echo "Certificate: $IDENTITY (valid until $EXPIRES)"

# The API key, checked against Apple with a read-only call.
KEY_ID=$(basename "$P8" .p8 | sed -n 's/^AuthKey_//p')
[[ -n $KEY_ID ]] || read -r -p "Key ID of $(basename "$P8"): " KEY_ID
read -r -p "Issuer ID (App Store Connect → Users and Access → Integrations): " ISSUER
xcrun notarytool history --key "$P8" --key-id "$KEY_ID" --issuer "$ISSUER" >/dev/null 2>&1 || die "Apple refused that key, key ID or issuer ID (it needs the Developer role or higher)"
echo "API key $KEY_ID: accepted by Apple"

echo
read -r -p "Store these as secrets of $REPO's \"$ENVIRONMENT\" environment? [y/N] " yes
[[ $yes == [yY] ]] || die "nothing stored"

set_secret() { gh secret set "$1" --repo "$REPO" --env "$ENVIRONMENT" >/dev/null; }
base64 -i "$P12" | tr -d '\n' | set_secret APPLE_CERTIFICATE
printf '%s' "$P12_PASSWORD" | set_secret APPLE_CERTIFICATE_PASSWORD
printf '%s' "$IDENTITY" | set_secret APPLE_SIGNING_IDENTITY
printf '%s' "$KEY_ID" | set_secret APPLE_API_KEY
printf '%s' "$ISSUER" | set_secret APPLE_API_ISSUER
set_secret APPLE_API_KEY_P8 < "$P8"
unset P12_PASSWORD

echo "Stored: $(gh secret list --repo "$REPO" --env "$ENVIRONMENT" --json name --jq 'map(.name) | join(", ")')"
cat <<'DONE'

Next:
  - Keep the .p12, its password and the .p8 in your password manager. Apple lets you download
    a .p8 only once.
  - Then delete the loose copies (and empty the Trash if they went there).
  - The next v* tag signs, notarizes and checks the macOS build.
DONE
