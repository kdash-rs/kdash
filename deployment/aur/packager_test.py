#!/usr/bin/env python3
"""Unit tests for deployment/aur/packager.py.

Run directly:
    python3 deployment/aur/packager_test.py

Wired into .github/workflows/ci.yml as a release-readiness step so
template or argv regressions surface on every PR, not just at release
time.

This file is excluded from the published crate via Cargo.toml's
`exclude = ["deployment/*"]`.
"""

from __future__ import annotations

import os
import re
import sys
import tempfile
import unittest
from pathlib import Path

# Import the module under test from the same directory.
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import packager  # noqa: E402

SOURCE_TEMPLATE = str(HERE / "kdash" / "PKGBUILD.template")
BIN_TEMPLATE = str(HERE / "kdash-bin" / "PKGBUILD.template")


class _RenderTestBase(unittest.TestCase):
    """Shared scratch-file lifecycle. Subclasses define _render()."""

    def setUp(self) -> None:
        self.tempfiles: list[str] = []

    def tearDown(self) -> None:
        for f in self.tempfiles:
            try:
                os.unlink(f)
            except OSError:
                pass

    def _new_outfile(self) -> str:
        out = tempfile.NamedTemporaryFile(mode="w", suffix=".PKGBUILD", delete=False)
        out.close()
        self.tempfiles.append(out.name)
        return out.name


class TestRenderSource(_RenderTestBase):
    def _render(self, **overrides):
        args = {
            "version": "0.0.1",
            "template_path": SOURCE_TEMPLATE,
            "output_path": self._new_outfile(),
            "sha_source": "a" * 64,
        }
        args.update(overrides)
        return packager.render_source(**args)

    # --- happy path -----------------------------------------------------

    def test_substitutes_version_and_sha(self):
        rendered = self._render(version="1.2.3", sha_source="b" * 64)
        self.assertIn("pkgver=1.2.3", rendered)
        self.assertIn("sha256sums=('" + ("b" * 64) + "')", rendered)

    def test_leading_v_is_stripped(self):
        rendered = self._render(version="v0.0.1")
        self.assertIn("pkgver=0.0.1", rendered)
        self.assertNotIn("pkgver=v0.0.1", rendered)

    def test_no_template_placeholders_survive(self):
        rendered = self._render()
        # Templates use the braced `${name}` form for substitution targets so
        # they don't collide with bash's bare `$pkgname` etc. which are
        # legitimate runtime references and MUST remain in the output. We
        # only assert that the braced template placeholders are gone.
        for placeholder in ("${version}", "${sha_source}"):
            self.assertNotIn(placeholder, rendered, f"{placeholder!r} survived")
        # And, defensively, no other braced placeholder slipped in.
        survivors = re.findall(r"\$\{[a-zA-Z_][a-zA-Z0-9_]*\}", rendered)
        self.assertEqual(survivors, [], f"unresolved braced placeholders: {survivors!r}")

    # --- input shape rejection -----------------------------------------

    def test_prerelease_version_is_rejected(self):
        # AUR pkgver disallows `-`, so the packager refuses prerelease
        # versions even though the homebrew packager accepts them.
        for bad in ["0.0.1-rc1", "1.0.0-alpha.1", "0.0.1-beta.0"]:
            with self.subTest(version=bad):
                with self.assertRaises(SystemExit) as cm:
                    self._render(version=bad)
                self.assertEqual(cm.exception.code, 2)

    def test_invalid_version_is_rejected(self):
        for bad in ["", "not-a-version", "0.0", "0.0.1.5", "0.0.1+meta", "0.0.1 ; rm -rf"]:
            with self.subTest(version=bad):
                with self.assertRaises(SystemExit) as cm:
                    self._render(version=bad)
                self.assertEqual(cm.exception.code, 2)

    def test_invalid_sha256_is_rejected(self):
        for bad in ["", "tooshort", "a" * 63, "a" * 65, "g" * 64, "a" * 32 + "!" * 32]:
            with self.subTest(sha=bad):
                with self.assertRaises(SystemExit) as cm:
                    self._render(sha_source=bad)
                self.assertEqual(cm.exception.code, 2)

    def test_missing_template_path_raises(self):
        with self.assertRaises(FileNotFoundError):
            self._render(template_path="/nonexistent.template")


class TestRenderBin(_RenderTestBase):
    def _render(self, **overrides):
        args = {
            "version": "0.0.1",
            "template_path": BIN_TEMPLATE,
            "output_path": self._new_outfile(),
            "sha_x86_64_linux_gnu": "a" * 64,
            "sha_aarch64_linux_gnu": "b" * 64,
        }
        args.update(overrides)
        return packager.render_bin(**args)

    def test_substitutes_per_arch_shas(self):
        rendered = self._render(
            sha_x86_64_linux_gnu="c" * 64, sha_aarch64_linux_gnu="d" * 64
        )
        self.assertIn("sha256sums_x86_64=('" + ("c" * 64) + "')", rendered)
        self.assertIn("sha256sums_aarch64=('" + ("d" * 64) + "')", rendered)

    def test_no_template_placeholders_survive(self):
        rendered = self._render()
        for placeholder in (
            "${version}",
            "${sha_x86_64_linux_gnu}",
            "${sha_aarch64_linux_gnu}",
        ):
            self.assertNotIn(placeholder, rendered, f"{placeholder!r} survived")
        survivors = re.findall(r"\$\{[a-zA-Z_][a-zA-Z0-9_]*\}", rendered)
        self.assertEqual(survivors, [], f"unresolved braced placeholders: {survivors!r}")

    def test_bad_x86_sha_rejected(self):
        with self.assertRaises(SystemExit) as cm:
            self._render(sha_x86_64_linux_gnu="nope")
        self.assertEqual(cm.exception.code, 2)

    def test_bad_aarch64_sha_rejected(self):
        with self.assertRaises(SystemExit) as cm:
            self._render(sha_aarch64_linux_gnu="nope")
        self.assertEqual(cm.exception.code, 2)


class TestLeftoverPlaceholderCatch(_RenderTestBase):
    def test_unsubstituted_placeholder_is_caught(self):
        """A template that references an unknown $placeholder must fail."""
        bad_template = tempfile.NamedTemporaryFile(
            mode="w", suffix=".PKGBUILD.template", delete=False
        )
        try:
            bad_template.write(
                "pkgname=kdash\n"
                "pkgver=${version}\n"
                "sha256sums=('${sha_unknown_target}')\n"  # not in mapping
            )
            bad_template.close()
            with self.assertRaises(SystemExit) as cm:
                packager.render_source(
                    version="0.0.1",
                    template_path=bad_template.name,
                    output_path=self._new_outfile(),
                    sha_source="a" * 64,
                )
            self.assertEqual(cm.exception.code, 2)
        finally:
            os.unlink(bad_template.name)


class TestArgvHandling(_RenderTestBase):
    def test_missing_subcommand_exits_2(self):
        with self.assertRaises(SystemExit) as cm:
            packager.main(["packager.py"])
        self.assertEqual(cm.exception.code, 2)

    def test_unknown_subcommand_exits_2(self):
        with self.assertRaises(SystemExit) as cm:
            packager.main(["packager.py", "kdash-git", "0.0.1"])
        self.assertEqual(cm.exception.code, 2)

    def test_source_wrong_argc_exits_2(self):
        with self.assertRaises(SystemExit) as cm:
            packager.main(["packager.py", "kdash", "0.0.1"])
        self.assertEqual(cm.exception.code, 2)

    def test_bin_wrong_argc_exits_2(self):
        with self.assertRaises(SystemExit) as cm:
            packager.main(["packager.py", "kdash-bin", "0.0.1", SOURCE_TEMPLATE])
        self.assertEqual(cm.exception.code, 2)

    def test_source_correct_argc_renders(self):
        out = self._new_outfile()
        packager.main(
            [
                "packager.py",
                "kdash",
                "0.0.1",
                SOURCE_TEMPLATE,
                out,
                "a" * 64,
            ]
        )
        rendered = Path(out).read_text()
        self.assertIn("pkgver=0.0.1", rendered)

    def test_bin_correct_argc_renders(self):
        out = self._new_outfile()
        packager.main(
            [
                "packager.py",
                "kdash-bin",
                "0.0.1",
                BIN_TEMPLATE,
                out,
                "a" * 64,
                "b" * 64,
            ]
        )
        rendered = Path(out).read_text()
        self.assertIn("pkgver=0.0.1", rendered)
        self.assertIn("sha256sums_x86_64=('" + ("a" * 64) + "')", rendered)
        self.assertIn("sha256sums_aarch64=('" + ("b" * 64) + "')", rendered)


if __name__ == "__main__":
    unittest.main(verbosity=2)
