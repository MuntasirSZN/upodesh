#!/usr/bin/env python3
"""Regenerate `data/frequency.json` from the Leipzig Corpora Collection.

The output is a JSON object mapping a Bengali surface form to its summed token
count across the configured corpora. Only words present in the
OpenBangla-Keyboard dictionary are kept, because every other entry would just
be noise for the correction index.

Licence of the derived data: CC BY 4.0, see `data/frequency.LICENSE`.
"""

from __future__ import annotations

import json
import tarfile
import tempfile
import unicodedata
import urllib.request
from collections import Counter
from pathlib import Path

BASE = "https://downloads.wortschatz-leipzig.de/corpora"
CORPORA = [
    "ben_news_2020_300K",
    "ben_wikipedia_2021_300K",
]

ROOT = Path(__file__).resolve().parent.parent
DICTIONARY = ROOT / "data/openbangla-keyboard/data/dictionary.json"
OUTPUT = ROOT / "data/frequency.json"


def dictionary_words() -> set[str]:
    with DICTIONARY.open(encoding="utf-8") as handle:
        buckets = json.load(handle)
    return {
        unicodedata.normalize("NFC", word)
        for words in buckets.values()
        for word in words
    }


def read_word_counts(path: Path) -> Counter[str]:
    """Read a Leipzig `*-words.txt` file (`rank<TAB>word<TAB>count`)."""
    counts: Counter[str] = Counter()
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            fields = line.rstrip("\n").split("\t")
            if len(fields) != 3:
                continue
            _, word, raw_count = fields
            try:
                count = int(raw_count)
            except ValueError:
                continue
            counts[unicodedata.normalize("NFC", word)] += count
    return counts


def corpus_archive(corpus: str) -> Path:
    archive = tempfile.NamedTemporaryFile(
        prefix=f"{corpus}-", suffix=".tar.gz", delete=False
    )
    archive.close()
    target = Path(archive.name)
    url = f"{BASE}/{corpus}.tar.gz"
    print(f"downloading {url}")
    urllib.request.urlretrieve(url, target)
    return target


def corpus_word_file(archive: Path, corpus: str) -> Path:
    member = f"{corpus}/{corpus}-words.txt"
    extracted = Path(tempfile.mkdtemp(prefix=f"{corpus}-"))
    with tarfile.open(archive, "r:gz") as tar:
        tar.extract(member, path=extracted)
    return extracted / member


def main() -> None:
    words = dictionary_words()
    total: Counter[str] = Counter()

    for corpus in CORPORA:
        archive = corpus_archive(corpus)
        try:
            counts = read_word_counts(corpus_word_file(archive, corpus))
        finally:
            archive.unlink(missing_ok=True)
        matched = {word: count for word, count in counts.items() if word in words}
        print(
            f"{corpus}: {sum(counts.values())} tokens, {len(matched)} dictionary words"
        )
        total.update(matched)

    # Stable output keeps diffs reviewable.
    ordered = {word: total[word] for word in sorted(total)}
    with OUTPUT.open("w", encoding="utf-8") as handle:
        json.dump(ordered, handle, ensure_ascii=False, separators=(",", ":"))
    print(f"wrote {OUTPUT} ({len(ordered)} words)")


if __name__ == "__main__":
    main()
