#!/usr/bin/env python3
"""Extract small pinned installed-consumer input texts, without building an environment."""
# SPDX-License-Identifier: MPL-2.0
from __future__ import annotations
import argparse
import json
from pathlib import Path
import re
import reference
from witness_paths import admit_destinations

DIRECTORIES = ("tests/fixtures/basic", "tests/fixtures/terminal-recovery",
               "tests/fixtures/importance-sampling", "examples/dk")
DOCUMENTS = ("docs/software/getting-started.md", "docs/software/sampling.md")


def capture(upstream):
    upstream = Path(upstream).resolve()
    manifest = reference.load_manifest()
    before = reference.verify_source(upstream, manifest)
    files = {}
    for directory in DIRECTORIES:
        for path in sorted((upstream / directory).rglob("*")):
            name = path.relative_to(upstream).as_posix()
            if name in manifest["upstream"]["files"] and path.suffix in {".magna", ".json", ".py"}:
                files[name] = reference.relative_file(upstream, name).read_text()
    docs = {}
    for name in DOCUMENTS:
        text = reference.relative_file(upstream, name).read_text()
        docs[name] = dict(text_blocks=re.findall(r"```text\n(.*?)```", text, re.S),
                          python_blocks=re.findall(r"```python\n(.*?)```", text, re.S))
    if len(files) != 31 or [len(docs[n]["python_blocks"]) for n in DOCUMENTS] != [2, 3]:
        raise ValueError("original input/example inventory changed")
    paths = sorted(set(files) | set(docs))
    result = dict(schema="rmagna.original-installed-inputs.v1",
                  upstream_commit=manifest["upstream"]["commit"],
                  reference_sha256=reference.digest(reference.MANIFEST),
                  source_sha256={p: manifest["upstream"]["files"][p] for p in paths}, files=files, docs=docs)
    if reference.verify_source(upstream, manifest) != before:
        raise ValueError("source changed during installed-input extraction")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    try:
        destination, = admit_destinations([args.out], args.upstream, protected_roots=[reference.HERE])
        result = capture(args.upstream)
        with destination.open("x", encoding="utf-8") as stream:
            json.dump(result, stream, indent=2, ensure_ascii=False, allow_nan=False)
            stream.write("\n")
    except (ValueError, OSError) as error:
        parser.error(str(error))
    print(f"extracted {len(result['files'])} input files and {len(result['docs'])} tutorial documents")


if __name__ == "__main__":
    main()
