#!/usr/bin/env python3
"""Generate the PKGBUILD for the kdash-bin AUR package.

Usage:
    packager.py kdash-bin <version> <template_path> <output_path> \\
        <sha_x86_64_linux_gnu> <sha_aarch64_linux_gnu>

The kdash (source) and kdash-git packages are static PKGBUILDs -- they
have no template placeholders, so they are not rendered through this
script. The release jobs bump their `pkgver` and regenerate checksums
with `updpkgsums` instead.

Mirrors deployment/homebrew/packager.py: same string.Template +
safe_substitute approach, hardened with input shape checks and a
post-render assertion that no `$placeholder` survives. The version
regex is tighter than the homebrew one: PKGBUILD `pkgver` cannot
contain `-`, so prerelease versions are rejected at the packager
layer. The release workflow already gates AUR publishing on stable
tags only; this is the second line of defence.

This script is in `Cargo.toml`'s `exclude` list (via `deployment/*`)
so it does not ship in the published crate.
"""

from __future__ import annotations

import re
import sys
from string import Template

# Stable semver only -- no prerelease suffix. AUR pkgver disallows `-`.
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
SHA256_RE = re.compile(r"^[a-fA-F0-9]{64}$")

# Templates use the braced `${name}` form for substitution targets so they
# don't clash with bash's bare `$pkgname` / `$pkgdir` / `$srcdir` etc., which
# are legitimate runtime references in a PKGBUILD. The leftover-check scans
# only for surviving braced placeholders for the same reason.
_BRACED_LEFTOVER_RE = re.compile(r"\$\{[a-zA-Z_][a-zA-Z0-9_]*\}")


def _die(msg: str, code: int = 2) -> None:
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(code)


def _normalize_version(version: str) -> str:
    """Strip a leading 'v' and validate against VERSION_RE."""
    version = version.lstrip("v")
    if not VERSION_RE.match(version):
        _die(
            f"invalid version: {version!r} "
            f"(expected X.Y.Z; AUR pkgver disallows prerelease suffixes)"
        )
    return version


def _validate_sha(label: str, sha: str) -> None:
    if not SHA256_RE.match(sha):
        _die(f"invalid {label}: {sha!r} (expected 64 hex chars)")


def _render_template(template_path: str, output_path: str, mapping: dict) -> str:
    """Substitute `mapping` into the template at `template_path`.

    Asserts every `$placeholder` is resolved; writes to `output_path`;
    returns the rendered text.

    `safe_substitute` silently leaves unknown $placeholders in the
    output, so without this check a template typo (e.g.
    `$sha_arm64_linux`) would ship a broken PKGBUILD.
    """
    with open(template_path, "r", encoding="utf-8") as fh:
        template_src = fh.read()

    template = Template(template_src)
    rendered = template.safe_substitute(mapping)

    leftover = sorted(set(_BRACED_LEFTOVER_RE.findall(rendered)))
    if leftover:
        _die(
            "template has unresolved ${...} placeholders after substitution: "
            + ", ".join(leftover)
        )

    with open(output_path, "w", encoding="utf-8") as fh:
        fh.write(rendered)

    return rendered


def render_bin(
    version: str,
    template_path: str,
    output_path: str,
    sha_x86_64_linux_gnu: str,
    sha_aarch64_linux_gnu: str,
) -> str:
    """Render the kdash-bin PKGBUILD. Returns the rendered text."""
    version = _normalize_version(version)
    _validate_sha("sha_x86_64_linux_gnu", sha_x86_64_linux_gnu)
    _validate_sha("sha_aarch64_linux_gnu", sha_aarch64_linux_gnu)
    return _render_template(
        template_path,
        output_path,
        {
            "version": version,
            "sha_x86_64_linux_gnu": sha_x86_64_linux_gnu,
            "sha_aarch64_linux_gnu": sha_aarch64_linux_gnu,
        },
    )


# argv layout: subcommand + version + template + out + sha_x86_64 + sha_aarch64
_EXPECTED_ARGC = {
    "kdash-bin": 7,
}


def main(argv: list[str]) -> None:
    if len(argv) < 2:
        print(__doc__.strip() if __doc__ else "", file=sys.stderr)
        _die("missing subcommand (expected `kdash-bin`)")

    sub = argv[1]
    if sub not in _EXPECTED_ARGC:
        _die(f"unknown subcommand: {sub!r} (expected `kdash-bin`)")

    expected = _EXPECTED_ARGC[sub]
    if len(argv) != expected:
        print(__doc__.strip() if __doc__ else "", file=sys.stderr)
        _die(f"{sub}: expected {expected - 1} args, got {len(argv) - 1}")

    version = argv[2].strip()
    template_path = argv[3]
    output_path = argv[4]
    sha_x86_64 = argv[5].strip()
    sha_aarch64 = argv[6].strip()

    print("Generating PKGBUILD (kdash-bin)")
    print(f"     VERSION: {version}")
    print(f"     TEMPLATE PATH: {template_path}")
    print(f"     SAVING AT: {output_path}")
    print(f"     SHA x86_64-unknown-linux-gnu: {sha_x86_64}")
    print(f"     SHA aarch64-unknown-linux-gnu: {sha_aarch64}")

    rendered = render_bin(
        version, template_path, output_path, sha_x86_64, sha_aarch64
    )

    print("\n================== Generated PKGBUILD ==================\n")
    print(rendered)
    print("\n========================================================\n")


if __name__ == "__main__":
    main(sys.argv)
