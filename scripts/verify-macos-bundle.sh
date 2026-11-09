#!/usr/bin/env bash
# --test permits ad-hoc signing; only --distribution is a release gate.
set -euo pipefail
fail() { echo "Bundle verification failed: $*" >&2; exit 1; }
[[ $# == 3 ]] || fail 'Usage: verify-macos-bundle.sh --test|--distribution Netjes.app Netjes.dmg'
mode="$1"; app="$2"; dmg="$3"
[[ "$mode" == --test || "$mode" == --distribution ]] || fail 'Usage: verify-macos-bundle.sh --test|--distribution Netjes.app Netjes.dmg'
[[ -d "$app" && ! -L "$app" ]] || fail "App not found: $app"
[[ -f "$dmg" && ! -L "$dmg" ]] || fail "DMG not found: $dmg"
[[ $(uname -s) == Darwin ]] || fail 'macOS tools are required'
app="$(cd "$(dirname "$app")" && pwd)/$(basename "$app")"
dmg="$(cd "$(dirname "$dmg")" && pwd)/$(basename "$dmg")"

verify_app() {
  local bundle="$1" binary="$1/Contents/MacOS/netjes" arch details
  [[ -x "$binary" && ! -L "$binary" ]] || fail 'Missing netjes executable'
  lipo -verify_arch arm64 x86_64 "$binary" || fail 'Executable must be universal (arm64 and x86_64)'
  echo "Architectures: $(lipo -archs "$binary")"
  python3 - "$bundle/Contents/Info.plist" <<'PY'
import plistlib, sys
with open(sys.argv[1], 'rb') as f:
    p = plistlib.load(f)
assert p['CFBundleIdentifier'] == 'be.agentiek.netjes', 'Wrong bundle identifier'
assert p['CFBundleExecutable'] == 'netjes', 'Wrong executable'
assert p['CFBundleName'] == 'Netjes', 'Wrong app name'
assert p['LSMinimumSystemVersion'] in ('13.0', '13.0.0'), 'Wrong macOS minimum'
assert p['CFBundleShortVersionString'] == p['CFBundleVersion'], 'Bundle version mismatch'
for key in ('NSDownloadsFolderUsageDescription', 'NSDesktopFolderUsageDescription', 'NSDocumentsFolderUsageDescription'):
    assert p.get(key, '').strip(), f'Missing {key}'
print('Plist: Netjes', p['CFBundleShortVersionString'], 'minimum macOS', p['LSMinimumSystemVersion'])
PY
  # Check each Mach-O deployment target, not just the editable plist.
  for arch in arm64 x86_64; do
    otool -arch "$arch" -l "$binary" | python3 -c '
import re, sys
s = sys.stdin.read()
versions = re.findall(r"\bminos (\d+\.\d+(?:\.\d+)?)", s)
versions += re.findall(r"cmd LC_VERSION_MIN_MACOSX\s+cmdsize \d+\s+version (\d+\.\d+(?:\.\d+)?)", s)
assert len(versions) == 1 and versions[0] in ("13.0", "13.0.0"), "Wrong Mach-O minimum: " + str(versions)
print("Mach-O minimum:", versions[0])' || fail 'Invalid Mach-O macOS minimum'
    otool -arch "$arch" -L "$binary" | grep -q '/System/Library/Frameworks/QuickLookThumbnailing.framework/' || fail "Missing static Quick Look framework linkage for $arch"
  done
  codesign --verify --deep --strict --all-architectures --verbose=2 "$bundle" || fail 'Invalid app signature'
  if [[ "$mode" == --distribution ]]; then
    [[ "${APPLE_TEAM_ID:-}" =~ ^[A-Z0-9]{10}$ ]] || fail 'Developer ID Application verification requires APPLE_TEAM_ID'
    for arch in arm64 x86_64; do
      details=$(codesign -d --verbose=4 --arch "$arch" "$bundle" 2>&1) || fail 'Cannot inspect signature'
      [[ "$details" == *'Authority=Developer ID Application:'* && "$details" == *"TeamIdentifier=$APPLE_TEAM_ID"* && "$details" == *'(runtime)'* && "$details" == *'Timestamp='* ]] || fail 'Developer ID Application, expected team, hardened runtime and secure timestamp required'
    done
    xcrun stapler validate "$bundle" || fail 'Missing app notarization ticket'
    spctl --assess --type execute --verbose=2 "$bundle" || fail 'Gatekeeper rejected app'
  fi
}
verify_app "$app"
hdiutil verify "$dmg" || fail 'Invalid DMG checksum'
if [[ "$mode" == --distribution ]]; then
  codesign --verify --strict --verbose=2 "$dmg" || fail 'Invalid DMG signature'
  details=$(codesign -d --verbose=4 "$dmg" 2>&1) || fail 'Cannot inspect DMG signature'
  [[ "$details" == *'Authority=Developer ID Application:'* && "$details" == *"TeamIdentifier=$APPLE_TEAM_ID"* && "$details" == *'Timestamp='* ]] || fail 'Developer ID Application DMG signature required'
  xcrun stapler validate "$dmg" || fail 'Missing DMG notarization ticket'
  spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg" || fail 'Gatekeeper rejected DMG'
fi
mount=$(mktemp -d)
attached=false
cleanup() {
  if [[ "$attached" == true ]]; then hdiutil detach "$mount" >/dev/null; fi
  rmdir "$mount"
}
trap cleanup EXIT
hdiutil attach -readonly -nobrowse -mountpoint "$mount" "$dmg" >/dev/null || fail 'Cannot mount DMG'
attached=true
[[ -d "$mount/Netjes.app" && ! -L "$mount/Netjes.app" ]] || fail 'DMG has no Netjes.app'
[[ -L "$mount/Applications" && $(readlink "$mount/Applications") == /Applications ]] || fail 'DMG missing Applications installation link'
diff -qr "$app" "$mount/Netjes.app" || fail 'DMG app does not match verified app'
verify_app "$mount/Netjes.app"
echo "Verified $mode: $dmg (mounted app matches verified bundle)"
