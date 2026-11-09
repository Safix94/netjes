"""Exercise release preflight against real temporary Git tags/manifests (no Apple access)."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("check-macos-release.py")


class ReleasePreflightTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "src-tauri").mkdir()
        (self.root / "package.json").write_text('{"version":"1.0.0"}')
        (self.root / "src-tauri/tauri.conf.json").write_text('{"version":"1.0.0"}')
        (self.root / "src-tauri/Cargo.toml").write_text('[package]\nversion = "1.0.0"\n')
        for args in (["init", "-q"], ["add", "."],
                     ["-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                      "commit", "-qm", "fixture"], ["tag", "v1.0.0"]):
            subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)
        self.sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.root, text=True).strip()
        self.releases = self.root / "releases.json"
        self.releases.write_text("[[]]")
        self.env = dict(os.environ, RELEASE_TAG="v1.0.0", SOURCE_SHA=self.sha,
                        APPLE_CERTIFICATE="fixture", APPLE_CERTIFICATE_PASSWORD="fixture",
                        APPLE_SIGNING_IDENTITY="Developer ID Application: Test (ABCDEFGHIJ)",
                        APPLE_TEAM_ID="ABCDEFGHIJ", APPLE_API_KEY="ABCDEFGHIJ",
                        APPLE_API_ISSUER="00000000-0000-0000-0000-000000000000",
                        APPLE_API_PRIVATE_KEY="fixture")

    def run_check(self, expected, message=""):
        result = subprocess.run([sys.executable, str(SCRIPT), str(self.releases)],
                                cwd=self.root, env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        self.assertIn(message, result.stdout + result.stderr)

    def test_exact_tag_commit_and_versions_pass(self):
        self.run_check(0, "Release preflight passed")

    def test_missing_secret_fails_closed(self):
        del self.env["APPLE_API_PRIVATE_KEY"]
        self.run_check(1, "Missing required secrets: APPLE_API_PRIVATE_KEY")

    def test_shell_payload_in_tag_is_rejected(self):
        self.env["RELEASE_TAG"] = 'v1.0.0$(touch injected)'
        self.run_check(1, "tag must be")
        self.assertFalse((self.root / "injected").exists())

    def test_branch_name_is_not_an_exact_commit(self):
        self.env["SOURCE_SHA"] = "main"
        self.run_check(1, "40-character")

    def test_wrong_checkout_is_rejected(self):
        self.env["SOURCE_SHA"] = "0" * 40
        self.run_check(1, "Checkout/tag/source commit mismatch")

    def test_version_disagreement_is_rejected(self):
        (self.root / "package.json").write_text('{"version":"1.0.1"}')
        self.run_check(1, "Version mismatch")

    def test_existing_public_or_draft_release_is_never_mutated(self):
        for draft in (False, True):
            with self.subTest(draft=draft):
                self.releases.write_text(json.dumps([[{"tag_name": "v1.0.0", "draft": draft}]]))
                self.run_check(1, "Release already exists")

    def test_adhoc_and_wrong_team_identity_are_rejected(self):
        for identity in ("-", "Apple Development: Test (ABCDEFGHIJ)",
                         "Developer ID Application: Test (OTHERTEAM1)"):
            with self.subTest(identity=identity):
                self.env["APPLE_SIGNING_IDENTITY"] = identity
                self.run_check(1, "Developer ID Application")


if __name__ == "__main__":
    unittest.main()
