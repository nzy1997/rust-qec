#!/usr/bin/env python3
"""Serve a local site with enough pending connections for parallel browsers."""

import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class SiteServer(ThreadingHTTPServer):
    # Browsers preconnect several sockets while loading pages and WASM assets.
    request_queue_size = 128


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bind", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--directory", type=Path, default=Path(__file__).resolve().parents[1] / "_site")
    args = parser.parse_args()
    handler = partial(SimpleHTTPRequestHandler, directory=str(args.directory.resolve()))
    with SiteServer((args.bind, args.port), handler) as server:
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
