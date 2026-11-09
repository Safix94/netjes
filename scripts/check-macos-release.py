"""Fail closed before installing tools or creating signing credentials. Never prints secrets."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib


def check(releases_path):
    tag = os.environ.get("RELEASE_TAG", "")
    sha = os.environ.get("SOURCE_SHA", "")
    if not re.fullmatch(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", tag):
        raise ValueError("Release tag must be vMAJOR.MINOR.PATCH (no prerelease or shell syntax)")
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("Source must be an exact lowercase 40-character commit SHA")
    git = lambda ref: subprocess.check_output(
        ["git", "rev-parse", "--verify", ref], text=True).strip()
    if git("HEAD") != sha or git(f"refs/tags/{tag}^{{commit}}") != sha:
        raise ValueError("Checkout/tag/source commit mismatch")
    versions = [json.loads(Path(p).read_text(encoding="utf-8"))["version"]
                for p in ("package.json", "src-tauri/tauri.conf.json")]
    versions.append(tomllib.loads(Path("src-tauri/Cargo.toml").read_text(encoding="utf-8"))["package"]["version"])
    if any(version != tag[1:] for version in versions):
        raise ValueError("Version mismatch between tag, package.json, Cargo.toml and Tauri config")
    pages = json.loads(Path(releases_path).read_text(encoding="utf-8"))
    if not isinstance(pages, list) or any(not isinstance(page, list) for page in pages):
        raise ValueError("Invalid release listing")
    if any(release["tag_name"] == tag for page in pages for release in page):
        raise ValueError("Release already exists; no public or draft release may be mutated")
    required = ("APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "APPLE_SIGNING_IDENTITY",
                "APPLE_TEAM_ID", "APPLE_API_KEY", "APPLE_API_ISSUER", "APPLE_API_PRIVATE_KEY")
    missing = [key for key in required if not os.environ.get(key, "").strip()]
    if missing:
        raise ValueError("Missing required secrets: " + ", ".join(missing))
    team = os.environ["APPLE_TEAM_ID"]
    identity = os.environ["APPLE_SIGNING_IDENTITY"]
    if not re.fullmatch(r"[A-Z0-9]{10}", team) or not re.fullmatch(
            r"Developer ID Application: [^\r\n]+ \(" + re.escape(team) + r"\)", identity):
        raise ValueError("A Developer ID Application identity for APPLE_TEAM_ID is required")
    if not re.fullmatch(r"[A-Z0-9]{10}", os.environ["APPLE_API_KEY"]) or not re.fullmatch(
            r"[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}", os.environ["APPLE_API_ISSUER"]):
        raise ValueError("Invalid App Store Connect key ID or issuer ID")
    print(f"Release preflight passed: {tag} at {sha}; no existing release")


if __name__ == "__main__":
    try:
        if len(sys.argv) != 2:
            raise ValueError("Usage: check-macos-release.py releases-pages.json")
        check(sys.argv[1])
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"Release preflight failed: {error}", file=sys.stderr)
        sys.exit(1)
