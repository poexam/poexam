#!/usr/bin/env python3
#
# SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""Generate src/po/linebreak_table.rs from the libunistring sources.

The line breaking properties, the pair table and the character widths are the
ones used by GNU libunistring (and thus GNU gettext, e.g. msgcat), so that
poexam wraps the messages exactly like msgcat.

Usage (the libunistring sources are in its release tarball):

    tools/gen-linebreak-table.py /path/to/libunistring-x.y.z > src/po/linebreak_table.rs
    cargo fmt
"""

import re
import sys
from pathlib import Path

# Rust source using the line breaking classes: only the classes it uses are
# generated as constants.
LINEBREAK_SOURCE = Path(__file__).resolve().parent.parent / "src" / "po" / "linebreak.rs"

# Hangul syllables: alternating LV (H2) and LVT (H3) syllables, computed at
# runtime instead of stored in the table.
HANGUL_FIRST = 0xAC00
HANGUL_LAST = 0xD7A3


def parse_classes(header: str) -> dict[str, int]:
    """Return the line breaking classes (name without `LBP_` -> value)."""
    enum = header[header.index("enum") : header.index("};")]
    return {name: int(value) for name, value in re.findall(r"LBP_(\w+)\s*=\s*(\d+)", enum)}


def parse_pair_table(source: str) -> list[list[str]]:
    """Return the pair table (rows of `D`, `I`, `P`)."""
    table = source[source.index("unilbrk_table[43][43]") :]
    rows = re.findall(r"\{([DIP,\s]+)\}", table)
    result = [re.findall(r"[DIP]", row) for row in rows]
    assert len(result) == 43 and all(len(row) == 43 for row in result)
    return result


def parse_properties(source: str, classes: dict[str, int]) -> list[tuple[int, int]]:
    """Return the ranges (first code point, (class << 1) | east_asian)."""
    header = dict(re.findall(r"#define lbrkprop_header_(\d) (\d+)", source))
    shift1, size1, shift2, mask2, mask3 = (int(header[str(i)]) for i in range(5))
    body = source[source.index("unilbrkprop =") :]
    groups = re.findall(r"\{([^{}]*)\}", body)
    level1 = [int(x) for x in re.findall(r"-?\d+", groups[0])]
    level2 = [int(x) for x in re.findall(r"-?\d+", groups[1])]
    level3 = [
        (classes[name] << 1) | int(ea)
        for name, ea in re.findall(r"\(LBP_(\w+)<<1\)\|(\d)", groups[2])
    ]
    unknown = classes["XX"] << 1

    def lookup(uc: int) -> int:
        index1 = uc >> shift1
        if index1 < size1 and level1[index1] >= 0:
            lookup2 = level2[level1[index1] + ((uc >> shift2) & mask2)]
            if lookup2 >= 0:
                return level3[lookup2 + (uc & mask3)]
        return unknown

    hangul = (classes["H2"] << 1) | 1
    ranges = []
    previous = None
    for uc in range(0x110000):
        value = lookup(uc)
        if HANGUL_FIRST <= uc <= HANGUL_LAST:
            expected = classes["H2"] if (uc - HANGUL_FIRST) % 28 == 0 else classes["H3"]
            assert value == (expected << 1) | 1, f"unexpected Hangul property: U+{uc:04X}"
            value = hangul
        if value != previous:
            ranges.append((uc, value))
            previous = value
    return ranges


def c_entries(group: str) -> list[int]:
    """Return the entries of a C array: -1, or the offset after the last `+`."""
    return [
        -1 if entry.strip() == "-1" else int(entry.rsplit("+", 1)[-1])
        for entry in group.split(",")
        if entry.strip()
    ]


def parse_widths(width0: str, width2: str) -> list[tuple[int, int]]:
    """Return the ranges (first code point, width) of the characters, as
    computed by the function `uc_width` of libunistring for UTF-8 (control
    characters, with width -1, are ignored by the line breaking: width 0)."""
    width0 = re.sub(r"/\*.*?\*/", "", width0, flags=re.DOTALL)
    width2 = re.sub(r"/\*.*?\*/", "", width2, flags=re.DOTALL)
    ind_src = width0[width0.index("nonspacing_table_ind") :]
    nonspacing_ind = [int(x) for x in re.findall(r"-?\d+", ind_src[ind_src.index("{") : ind_src.index("}")])]
    data_src = width0[width0.index("nonspacing_table_data") :]
    nonspacing_data = [
        int(x, 16) for x in re.findall(r"0x([0-9a-f]{2})\b", data_src[data_src.index("{") : data_src.index("};")])
    ]
    body = width2[width2.index("u_width2 =") :]
    groups = re.findall(r"\{([^{}]*)\}", body)
    # Offsets of level2 (in shorts) and level3 (in ints) are relative to the
    # start of the arrays: keep only the offset after the last "+".
    level1 = c_entries(groups[1])
    level2 = c_entries(groups[2])
    level3 = [int(x, 16) for x in re.findall(r"0x([0-9A-F]+)U", groups[3])]

    def is_wide(uc: int) -> bool:
        index1 = uc >> 16
        if index1 < len(level1) and level1[index1] >= 0:
            lookup2 = level2[level1[index1] + ((uc >> 9) & 127)]
            if lookup2 >= 0:
                return (level3[lookup2 + ((uc >> 5) & 15)] >> (uc & 31)) & 1 == 1
        return False

    def width(uc: int) -> int:
        if (uc >> 9) < len(nonspacing_ind):
            ind = nonspacing_ind[uc >> 9]
            if ind >= 0 and (nonspacing_data[64 * ind + ((uc >> 3) & 63)] >> (uc & 7)) & 1:
                return 0
        elif (uc >> 9) == (0xE0000 >> 9):
            if uc >= 0xE0100:
                if uc <= 0xE01EF:
                    return 0
            elif (uc <= 0xE007F) if uc >= 0xE0020 else (uc == 0xE0001):
                return 0
        return 2 if is_wide(uc) else 1

    ranges = []
    previous = None
    for uc in range(0x110000):
        value = width(uc)
        if value != previous:
            ranges.append((uc, value))
            previous = value
    return ranges


def hex_literal(value: int) -> str:
    """Return the hexadecimal Rust literal, with `_` every 4 digits."""
    digits = f"{value:X}"
    groups = []
    while len(digits) > 4:
        groups.insert(0, digits[-4:])
        digits = digits[:-4]
    groups.insert(0, digits)
    return "0x" + "_".join(groups)


def wrap_numbers(values: list[str], indent: str = "    ", width: int = 100) -> str:
    """Return the values separated by commas, wrapped to `width` columns."""
    lines = []
    line = indent
    for value in values:
        item = f"{value},"
        if len(line) + len(item) + 1 > width:
            lines.append(line.rstrip())
            line = indent
        line += item + " "
    lines.append(line.rstrip())
    return "\n".join(lines)


def main() -> None:
    """Generate the Rust table."""
    root = Path(sys.argv[1]) / "lib" / "unilbrk"
    classes = parse_classes((root / "lbrktables.h").read_text())
    pairs = parse_pair_table((root / "lbrktables.c").read_text())
    prop_source = (root / "lbrkprop1.h").read_text() + (root / "lbrkprop2.h").read_text()
    unicode_version = re.search(r"for Unicode (\S+)\.\s", prop_source).group(1)
    ranges = parse_properties(prop_source, classes)
    width_dir = Path(sys.argv[1]) / "lib" / "uniwidth"
    widths = parse_widths((width_dir / "width0.h").read_text(), (width_dir / "width2.h").read_text())
    version = Path(sys.argv[1]).resolve().name

    out = []
    # REUSE-IgnoreStart
    out.append("// SPDX-FileCopyrightText: 2000-2026 Free Software Foundation, Inc.")
    out.append("// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>")
    out.append("//")
    out.append("// SPDX-License-Identifier: GPL-3.0-or-later")
    # REUSE-IgnoreEnd
    out.append("")
    out.append("// DO NOT EDIT! Generated by tools/gen-linebreak-table.py")
    out.append(f"// from {version} (Unicode {unicode_version}).")
    out.append("")
    out.append("//! Line breaking properties, pair table and character widths of GNU")
    out.append("//! libunistring.")
    out.append("")
    used = set(re.findall(r"\bLBP_(\w+)\b", LINEBREAK_SOURCE.read_text()))
    for name, value in sorted(classes.items(), key=lambda item: item[1]):
        if name in used:
            out.append(f"pub const LBP_{name}: u8 = {value};")
    out.append("")
    out.append("/// First code point of the Hangul syllables (alternating LV and LVT).")
    out.append(f"pub const HANGUL_FIRST: u32 = 0x{HANGUL_FIRST:04X};")
    out.append("")
    out.append("/// Last code point of the Hangul syllables.")
    out.append(f"pub const HANGUL_LAST: u32 = 0x{HANGUL_LAST:04X};")
    out.append("")
    ascii = [next(value for start, value in reversed(ranges) if start <= uc) for uc in range(128)]
    out.append("/// Property of the ASCII characters: `(class << 1) | east_asian`.")
    out.append("pub const ASCII_PROPS: [u8; 128] = [")
    out.append(wrap_numbers([str(value) for value in ascii]))
    out.append("];")
    out.append("")
    out.append("/// First code point of each range of characters with the same property.")
    out.append(f"pub const RANGE_STARTS: [u32; {len(ranges)}] = [")
    out.append(wrap_numbers([hex_literal(start) for start, _ in ranges]))
    out.append("];")
    out.append("")
    out.append("/// Property of each range: `(class << 1) | east_asian`.")
    out.append(f"pub const RANGE_PROPS: [u8; {len(ranges)}] = [")
    out.append(wrap_numbers([str(value) for _, value in ranges]))
    out.append("];")
    out.append("")
    out.append("/// First code point of each range of characters with the same width.")
    out.append(f"pub const WIDTH_STARTS: [u32; {len(widths)}] = [")
    out.append(wrap_numbers([hex_literal(start) for start, _ in widths]))
    out.append("];")
    out.append("")
    out.append("/// Width of each range (columns): 0, 1 or 2.")
    out.append(f"pub const WIDTH_VALUES: [u8; {len(widths)}] = [")
    out.append(wrap_numbers([str(value) for _, value in widths]))
    out.append("];")
    out.append("")
    out.append("/// Direct break opportunity.")
    out.append("pub const D: u8 = 1;")
    out.append("")
    out.append("/// Indirect break opportunity (only after spaces).")
    out.append("pub const I: u8 = 2;")
    out.append("")
    out.append("/// Prohibited break.")
    out.append("pub const P: u8 = 3;")
    out.append("")
    out.append("/// Pair table: break between a character of class `before` (row) and")
    out.append("/// a character of class `after` (column).")
    out.append("pub const PAIR_TABLE: [[u8; 43]; 43] = [")
    for row in pairs:
        out.append("    [" + ", ".join(row) + "],")
    out.append("];")
    print("\n".join(out))


if __name__ == "__main__":
    main()
