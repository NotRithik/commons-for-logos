#!/usr/bin/env python3
"""Download only the two pinned, already-deployed LEZ program files."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import urllib.request
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = 'NotRithik/commons-for-logos'
TAG = 'v0.1.0-rc.1'

def checked_item(family: str, value: dict) -> dict:
    if family not in ('allowlist', 'threshold') or value.get('file') != 'commons_' + family:
        raise ValueError('Unknown release program name')
    digest = value.get('sha256', '')
    if not isinstance(digest, str) or len(digest) != 64 or any(c not in '0123456789abcdef' for c in digest):
        raise ValueError('Invalid SHA256 pin')
    size = value.get('bytes')
    if type(size) is not int or not 0 < size < 2_000_000:
        raise ValueError('Invalid program size pin')
    return value

def check_bytes(data: bytes, item: dict) -> None:
    if len(data) != item['bytes'] or hashlib.sha256(data).hexdigest() != item['sha256']:
        raise ValueError('Downloaded program does not match the release manifest')

def fetch(item: dict, destination: Path) -> None:
    output = destination / item['file']
    if output.exists() or output.is_symlink():
        if output.is_symlink() or not output.is_file():
            raise ValueError('Existing program path is not a regular file')
        check_bytes(output.read_bytes(), item)
        print('Verified existing ' + item['file'])
        return
    url = f'https://github.com/{REPOSITORY}/releases/download/{TAG}/{item["file"]}'
    request = urllib.request.Request(url, headers={'User-Agent': 'commons-release-verifier/0.1'})
    with urllib.request.urlopen(request, timeout=45) as response:
        final = urlsplit(response.geturl())
        if final.scheme != 'https' or not (final.hostname == 'github.com' or str(final.hostname).endswith('.githubusercontent.com')):
            raise ValueError('Unexpected release download destination')
        data = response.read(item['bytes'] + 1)
    check_bytes(data, item)
    temp = None
    try:
        with tempfile.NamedTemporaryFile(prefix='verified-program-', dir=destination, delete=False) as stream:
            temp = Path(stream.name)
            stream.write(data)
        temp.replace(output)
    finally:
        if temp is not None and temp.exists(): temp.unlink()
    print('Downloaded and verified ' + item['file'])

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', type=Path, default=ROOT/'out/published-programs')
    args = parser.parse_args()
    destination = args.destination.resolve()
    if destination == ROOT or not destination.is_relative_to(ROOT):
        parser.error('destination must be inside this checkout')
    manifest = json.loads((ROOT/'release/manifest.json').read_text())
    if manifest.get('release_tag') != TAG: raise ValueError('Release tag does not match this fetcher')
    items = [checked_item(family, manifest['programs'][family]) for family in ('allowlist','threshold')]
    destination.mkdir(parents=True, exist_ok=True)
    for item in items: fetch(item, destination)

if __name__ == '__main__':
    main()
