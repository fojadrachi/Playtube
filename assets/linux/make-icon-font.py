"""Baut assets/PlaytubeIcons.ttf: die Symbole der Oberfläche für Linux.

Unter Windows zeichnet Playtube seine Symbole mit der Systemschrift "Segoe MDL2 Assets", die es
unter Linux nicht gibt. Diese kleine Schrift legt passende Symbole aus den freien Material Icons
(Google, Apache License 2.0) auf DIESELBEN Zeichencodes - die .slint-Dateien bleiben unverändert,
unter Linux wird nur die Schriftfamilie "Playtube Icons" gewählt (src/linux.rs).

Aufruf (fonttools nötig: pip install fonttools):
    python assets/linux/make-icon-font.py MaterialIcons-Regular.ttf MaterialIcons-Regular.codepoints
Quelle: https://github.com/google/material-design-icons/tree/master/font
"""

import sys
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

# Segoe-MDL2-Zeichencode -> Name in Material Icons
MAPPING = {
    0xE708: "bedtime",               # QuietHours (Schlaf-Timer)
    0xE70E: "keyboard_arrow_up",     # ChevronUp
    0xE710: "add",                   # Add
    0xE711: "close",                 # Cancel
    0xE721: "search",                # Search
    0xE72A: "arrow_forward",         # Forward
    0xE72B: "arrow_back",            # Back
    0xE72C: "refresh",               # Refresh
    0xE734: "star_border",           # FavoriteStar
    0xE73F: "picture_in_picture_alt",  # BackToWindow (Mini-Player)
    0xE740: "fullscreen",            # FullScreen
    0xE767: "volume_up",             # Volume
    0xE768: "play_arrow",            # Play
    0xE769: "pause",                 # Pause
    0xE80F: "home",                  # Home
    0xE81C: "history",               # History
    0xE892: "skip_previous",         # Previous
    0xE893: "skip_next",             # Next
    0xE8B1: "shuffle",               # Shuffle
    0xE8D6: "music_note",            # Audio
    0xE8ED: "repeat_one",            # RepeatOne
    0xE8EE: "repeat",                # RepeatAll
    0xE8F1: "library_music",         # Library
    0xE8FD: "format_list_bulleted",  # BulletedList
    0xE995: "graphic_eq",            # Volume3 (läuft gerade)
    0xEB51: "favorite_border",       # Heart
    0xEB52: "favorite",              # HeartFill
}

FAMILY = "Playtube Icons"


def main() -> None:
    font_path, codepoints_path = Path(sys.argv[1]), Path(sys.argv[2])
    out = Path(__file__).resolve().parent.parent / "PlaytubeIcons.ttf"
    codepoints = {}
    for line in codepoints_path.read_text(encoding="utf-8").splitlines():
        name, _, code = line.partition(" ")
        codepoints.setdefault(name, int(code, 16))

    font = TTFont(font_path)
    cmap = font.getBestCmap()
    new_map = {}
    for target, name in MAPPING.items():
        glyph = cmap[codepoints[name]]
        new_map[target] = glyph
    for table in font["cmap"].tables:
        if table.isUnicode():
            table.cmap = dict(new_map)

    options = subset.Options()
    options.layout_features = []
    options.name_IDs = ["*"]
    options.notdef_outline = True
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=list(new_map))
    subsetter.subset(font)

    for record in font["name"].names:
        if record.nameID in (1, 16):
            record.string = FAMILY
        elif record.nameID in (4,):
            record.string = FAMILY
        elif record.nameID == 6:
            record.string = "PlaytubeIcons-Regular"
    font.save(out)
    print(f"{out} ({out.stat().st_size} Bytes, {len(new_map)} Symbole)")


if __name__ == "__main__":
    main()
