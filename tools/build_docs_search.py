#!/usr/bin/env python3
"""Index rendered documentation, excluding navigation, scripts and styles."""
import argparse
import json
from html.parser import HTMLParser
from pathlib import Path


class Document(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.in_main = False
        self.in_title = False
        self.skip = 0
        self.title = []
        self.parts = []

    def handle_starttag(self, tag, attrs):
        if tag == 'main':
            self.in_main = True
        if tag == 'title':
            self.in_title = True
        if tag in ('script', 'style', 'noscript'):
            self.skip += 1

    def handle_endtag(self, tag):
        if tag == 'main':
            self.in_main = False
        if tag == 'title':
            self.in_title = False
        if tag in ('script', 'style', 'noscript'):
            self.skip = max(0, self.skip - 1)

    def handle_data(self, text):
        if not self.skip:
            if self.in_title:
                self.title.append(text)
            if self.in_main:
                self.parts.append(text)


def build(site_root):
    records = []
    for path in sorted(site_root.rglob('index.html')):
        relative = path.relative_to(site_root).as_posix()
        if relative.startswith(('gallery/', 'examples/', 'docs/')):
            continue
        document = Document()
        document.feed(path.read_text(encoding='utf-8'))
        body = ' '.join(' '.join(document.parts).split())
        if body:
            records.append({'title': ''.join(document.title).strip(),
                            'path': relative.removesuffix('index.html'), 'text': body})
    destination = site_root / 'data' / 'docs-search.json'
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(records, ensure_ascii=False), encoding='utf-8')
    return records


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--site-root', type=Path, required=True)
    args = parser.parse_args()
    print(f'Indexed {len(build(args.site_root))} documentation pages')
