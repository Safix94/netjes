#!/usr/bin/env bash
# Integration checks use the real CI-built bundle and disk image, never source inspection.
set -euo pipefail
verifier="$(cd "$(dirname "$0")" && pwd)/verify-macos-bundle.sh"
expect_failure() {
  local expected="$1"; shift
  local output
  if output=$(bash "$verifier" "$@" 2>&1); then
    echo "Unexpected verifier success: $*" >&2; exit 1
  fi
  if [[ "$output" != *"$expected"* ]]; then
    echo "Wrong failure; expected '$expected': $output" >&2; exit 1
  fi
  echo "PASS rejected: $expected"
}
expect_failure 'Usage:'
expect_failure 'Usage:' --unsafe a b
expect_failure 'App not found' --test /missing/Netjes.app /missing/Netjes.dmg
if [[ $# == 0 ]]; then exit 0; fi
[[ $# == 2 ]] || { echo 'Usage: test-verify-macos-bundle.sh [app dmg]' >&2; exit 1; }
app="$1"; dmg="$2"
bash "$verifier" --test "$app" "$dmg"
APPLE_TEAM_ID=ABCDEFGHIJ expect_failure 'Developer ID Application' --distribution "$app" "$dmg"
expect_failure 'DMG not found' --test "$app" /missing/Netjes.dmg
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
ditto "$app" "$scratch/Netjes.app"
binary="$scratch/Netjes.app/Contents/MacOS/netjes"
lipo "$binary" -thin arm64 -output "$scratch/thin"
cp "$scratch/thin" "$binary"
codesign --force --deep --sign - "$scratch/Netjes.app"
expect_failure 'universal' --test "$scratch/Netjes.app" "$dmg"
rm -rf "$scratch/Netjes.app"
ditto "$app" "$scratch/Netjes.app"
printf 'tampered signed resources\n' > "$scratch/Netjes.app/Contents/Resources/verification-fixture.txt"
expect_failure 'signature' --test "$scratch/Netjes.app" "$dmg"
rm -rf "$scratch/Netjes.app"
ditto "$app" "$scratch/Netjes.app"
/usr/libexec/PlistBuddy -c 'Set :LSMinimumSystemVersion 14.0' "$scratch/Netjes.app/Contents/Info.plist"
codesign --force --deep --sign - "$scratch/Netjes.app"
expect_failure 'minimum' --test "$scratch/Netjes.app" "$dmg"
rm -rf "$scratch/Netjes.app"
ditto "$app" "$scratch/Netjes.app"
printf 'different bundle\n' > "$scratch/Netjes.app/Contents/Resources/verification-fixture.txt"
codesign --force --deep --sign - "$scratch/Netjes.app"
expect_failure 'does not match' --test "$scratch/Netjes.app" "$dmg"
echo 'All real-bundle verification integration checks passed.'
