#!/usr/bin/env python3
"""Offline link check for the built mdBook (docs/book by default).

Every relative `href` / `src` in the rendered HTML must resolve to a file
in the book, and every `#fragment` on a link to another page of the book
must name an `id` on that page. External links (http, https, mailto, ...)
are not fetched: this runs on every PR, so it stays offline and
deterministic.

Paths the docs workflow generates after `mdbook build` (the UI embed
bundle under assets/, the playground and the WASM package) are skipped,
since a plain `mdbook build` cannot produce them.

Usage: scripts/check-book-links.py [BOOK_DIR] [--site-url /datalogic-rs/]
"""
import html.parser
import os
import sys
from urllib.parse import unquote, urlsplit

# Produced by docs.yml after the mdBook build, not by mdBook itself.
GENERATED_PREFIXES = (
    "assets/datalogic-embed.",
    "playground/",
    "wasm/",
)


class Collector(html.parser.HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []
        self.ids = set()

    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name in ("id", "name") and value:
                self.ids.add(value)
            if value and ((name == "href" and tag in ("a", "link")) or
                          (name == "src" and tag in ("img", "script", "iframe"))):
                self.links.append(value)


def parse(path, cache):
    if path not in cache:
        c = Collector()
        with open(path, encoding="utf-8", errors="replace") as f:
            c.feed(f.read())
        cache[path] = c
    return cache[path]


def main(argv):
    book = "docs/book"
    site_url = "/datalogic-rs/"
    args = list(argv)
    while args:
        a = args.pop(0)
        if a == "--site-url":
            site_url = args.pop(0)
        else:
            book = a
    book = os.path.abspath(book)
    if not os.path.isdir(book):
        print(f"no book at {book}; run `mdbook build docs` first", file=sys.stderr)
        return 2

    cache = {}
    broken = []
    pages = 0
    for root, _, files in os.walk(book):
        for name in files:
            if not name.endswith(".html"):
                continue
            page = os.path.join(root, name)
            pages += 1
            for link in parse(page, cache).links:
                parts = urlsplit(link)
                if parts.scheme or link.startswith("//"):
                    continue  # external: not checked offline
                path = unquote(parts.path)
                if not path:
                    target = page  # same-page fragment
                elif path.startswith("/"):
                    if not path.startswith(site_url):
                        continue  # outside the book's site
                    target = os.path.join(book, path[len(site_url):])
                else:
                    target = os.path.normpath(os.path.join(os.path.dirname(page), path))
                rel = os.path.relpath(target, book).replace(os.sep, "/")
                if rel.startswith(GENERATED_PREFIXES) or rel + "/" in GENERATED_PREFIXES:
                    continue
                if os.path.isdir(target):
                    target = os.path.join(target, "index.html")
                if not os.path.exists(target):
                    broken.append(f"{os.path.relpath(page, book)}: {link} (no {rel})")
                    continue
                frag = unquote(parts.fragment)
                if frag and target.endswith(".html") and target != page:
                    if frag not in parse(target, cache).ids:
                        broken.append(f"{os.path.relpath(page, book)}: {link} (no #{frag})")

    for b in sorted(set(broken)):
        print(f"BROKEN {b}", file=sys.stderr)
    if broken:
        print(f"\n{len(set(broken))} broken link(s) in {pages} pages", file=sys.stderr)
        return 1
    print(f"check-book-links: OK ({pages} pages)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
