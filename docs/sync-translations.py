#!/usr/bin/env python3
"""Refresh the gettext catalogues from the English book.

English is the source of truth: chapters live in `src/`, translations in
`po/<lang>.po`. After editing a chapter, run this to extract the messages again
and carry existing translations over onto the new template.

    python3 docs/sync-translations.py

It does the same job as `xgettext` + `msgmerge`, without needing the gettext
tools installed. Entries whose source text changed lose their translation and
fall back to English, which is the honest outcome — a stale translation of an
edited paragraph is worse than none.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

DOCS = Path(__file__).resolve().parent
LANGUAGES = ("ja",)


def extract_pot() -> None:
    """Run mdbook's xgettext renderer to regenerate po/messages.pot."""
    environment = dict(
        os.environ,
        MDBOOK_OUTPUT='{"xgettext": {"pot-file": "messages.pot", "granularity": 1}}',
    )
    subprocess.run(
        ["mdbook", "build", "-d", "po"],
        cwd=DOCS,
        env=environment,
        check=True,
        stdout=subprocess.DEVNULL,
    )


def split_blocks(text: str) -> tuple[str, list[str]]:
    """Return the header block and every message block of a PO file."""
    header, blocks = "", []
    for block in text.split("\n\n"):
        if not block.strip():
            continue
        if 'msgid ""' in block and '"Language:' in block:
            header = block
        else:
            blocks.append(block)
    return header, blocks


def unquote(raw: str) -> str:
    """Concatenate the quoted chunks of a PO value."""
    return "".join(re.findall(r'"((?:[^"\\]|\\.)*)"', raw))


def field(block: str, name: str) -> str:
    match = re.search(rf'^{name} ((?:"(?:[^"\\]|\\.)*"\n?)+)', block, re.M)
    return match.group(1).strip() if match else '""'


def merge(language: str) -> tuple[int, int]:
    pot_header, pot_blocks = split_blocks((DOCS / "po/messages.pot").read_text())
    catalogue = DOCS / f"po/{language}.po"

    known: dict[str, str] = {}
    header = ""
    if catalogue.exists():
        header, blocks = split_blocks(catalogue.read_text())
        known = {field(b, "msgid"): field(b, "msgstr") for b in blocks}
    if not header:
        header = pot_header.replace('"Language: en\\n"', f'"Language: {language}\\n"')

    merged, translated = [header], 0
    for block in pot_blocks:
        existing = known.get(field(block, "msgid"), '""')
        if unquote(existing).strip():
            block = re.sub(
                r'^msgstr ((?:"(?:[^"\\]|\\.)*"\n?)+)',
                "msgstr " + existing.replace("\\", "\\\\"),
                block,
                flags=re.M,
            )
            translated += 1
        merged.append(block)

    catalogue.write_text("\n\n".join(merged) + "\n")
    return translated, len(pot_blocks)


def main() -> int:
    try:
        extract_pot()
    except FileNotFoundError:
        print("mdbook is not on PATH: cargo install mdbook mdbook-i18n-helpers --locked")
        return 1

    for language in LANGUAGES:
        translated, total = merge(language)
        percent = 100 * translated // total if total else 0
        print(f"po/{language}.po: {translated}/{total} messages translated ({percent}%)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
