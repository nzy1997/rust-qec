#!/usr/bin/env python3
"""Index documentation sections with deep links and prose-only search previews."""
import argparse
import json
import re
from html.parser import HTMLParser
from pathlib import Path

VOID_TAGS = {'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input',
             'link', 'meta', 'param', 'source', 'track', 'wbr'}

def compact(parts):
    return ' '.join(''.join(parts).split())


class IdCollector(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = set()

    def handle_starttag(self, tag, attrs):
        value = dict(attrs).get('id')
        if value:
            self.ids.add(value)


class Document(HTMLParser):
    def __init__(self, ids=()):
        super().__init__(convert_charrefs=True)
        self.in_main = False
        self.in_title = False
        self.skipped = []
        self.dense = 0
        self.title = []
        self.sections = []
        self.current = {'heading': '', 'anchor': '', 'parts': [], 'prose': []}
        self.heading = None
        self.heading_id = ''
        self.ids = set(ids)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if self.skipped or tag in ('script', 'style', 'noscript', 'nav', 'aside') or 'data-toc-skip' in attrs:
            if tag not in VOID_TAGS:
                self.skipped.append(tag)
            return
        if tag == 'main':
            self.in_main = True
        if tag == 'title':
            self.in_title = True
        if not self.in_main:
            return
        if tag in ('pre', 'table'):
            self.dense += 1
        if attrs.get('id'):
            self.ids.add(attrs['id'])
        if tag in ('h2', 'h3', 'h4') or (tag == 'summary' and attrs.get('id')):
            self.flush()
            self.heading = []
            self.heading_id = attrs.get('id', '')
        if tag in ('p', 'li', 'div', 'pre', 'tr', 'td', 'br'):
            self.current['parts'].append(' ')
            if not self.dense:
                self.current['prose'].append(' ')

    def handle_endtag(self, tag):
        if self.skipped:
            if tag == self.skipped[-1]:
                self.skipped.pop()
            return
        if tag in ('h2', 'h3', 'h4', 'summary') and self.heading is not None:
            title = compact(self.heading)
            anchor = self.heading_id
            if not anchor:
                slug = re.sub(r'[^a-z0-9]+', '-', title.lower()).strip('-') or 'section'
                anchor = slug
                n = 2
                while anchor in self.ids:
                    anchor = f'{slug}-{n}'
                    n += 1
                self.ids.add(anchor)
            self.current.update(heading=title, anchor=anchor)
            self.heading = None
        if tag == 'main':
            self.flush()
            self.in_main = False
        if tag == 'title':
            self.in_title = False
        if self.in_main and tag in ('pre', 'table'):
            self.dense = max(0, self.dense - 1)
        if self.in_main and tag in ('p', 'li', 'div', 'pre', 'tr', 'td'):
            self.current['parts'].append(' ')
            if not self.dense:
                self.current['prose'].append(' ')

    def handle_data(self, text):
        if self.skipped:
            return
        if self.in_title:
            self.title.append(text)
        if self.in_main:
            if self.heading is not None:
                self.heading.append(text)
            else:
                self.current['parts'].append(text)
                if not self.dense:
                    self.current['prose'].append(text)

    def flush(self):
        if self.current['heading'] or compact(self.current['parts']):
            self.sections.append(self.current)
        self.current = {'heading': '', 'anchor': '', 'parts': [], 'prose': []}


def build(site_root):
    records = []
    for path in sorted(site_root.rglob('index.html')):
        relative = path.relative_to(site_root).as_posix()
        if relative == 'index.html' or relative.startswith(('gallery/', 'examples/', 'docs/', 'interactive/')):
            continue
        html = path.read_text(encoding='utf-8')
        ids = IdCollector()
        ids.feed(html)
        document = Document(ids.ids)
        document.feed(html)
        page_title = compact(document.title)
        for section in document.sections:
            text = compact(section['parts'])
            records.append({'title': section['heading'] or page_title,
                            'page_title': page_title,
                            'path': relative.removesuffix('index.html') + (f"#{section['anchor']}" if section['anchor'] else ''),
                            'text': text,
                            'excerpt': compact(section['prose'])})
    destination = site_root / 'data' / 'docs-search.json'
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(records, ensure_ascii=False), encoding='utf-8')
    return records


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--site-root', type=Path, required=True)
    args = parser.parse_args()
    print(f'Indexed {len(build(args.site_root))} documentation sections')
