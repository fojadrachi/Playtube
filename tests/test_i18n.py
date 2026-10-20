"""Uebersetzungen: jeder Text in beiden Sprachen, mit denselben Platzhaltern."""
from __future__ import annotations

import string
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from playtube import i18n  # noqa: E402


class TranslationTests(unittest.TestCase):
    def test_every_string_has_all_languages(self):
        for key, entry in i18n.STRINGS.items():
            with self.subTest(key=key):
                self.assertEqual(set(entry), set(i18n.LANGUAGES))

    def test_placeholders_match_between_languages(self):
        fmt = string.Formatter()
        for key, entry in i18n.STRINGS.items():
            fields = {lang: {f for _, f, _, _ in fmt.parse(text) if f} for lang, text in entry.items()}
            with self.subTest(key=key):
                self.assertEqual(fields["de"], fields["en"])

    def test_tr_uses_current_language_and_unknown_falls_back(self):
        previous = i18n._current
        try:
            i18n._current = "en"
            self.assertEqual(i18n.tr("tray.quit"), "Quit")
            i18n._current = "de"
            self.assertEqual(i18n.tr("tray.quit"), "Beenden")
            self.assertEqual(i18n.tr("gibt.es.nicht"), "gibt.es.nicht")
        finally:
            i18n._current = previous

    def test_resolve_language_keeps_known_codes(self):
        self.assertEqual(i18n.resolve_language("de"), "de")
        self.assertEqual(i18n.resolve_language("en"), "en")
        self.assertIn(i18n.resolve_language("xx"), i18n.LANGUAGES)


if __name__ == "__main__":
    unittest.main()
