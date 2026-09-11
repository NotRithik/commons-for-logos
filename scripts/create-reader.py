#!/usr/bin/env python3
"""Create a key-free Commons testnet reader. No network request or transaction."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import stat
import sys

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, help="New reader folder; it must not already exist")
    args = parser.parse_args()
    destination = args.directory.expanduser().absolute()
    if destination.exists() or destination.is_symlink():
        parser.error("That path already exists. Choose a new folder; existing wallets are never replaced.")
    if not destination.parent.is_dir():
        parser.error("The parent folder must already exist.")
    if any(parent.is_symlink() for parent in (destination.parent, *destination.parents)):
        parser.error("Choose a path without symbolic-link parents.")

    manifest = json.loads((ROOT / "release/manifest.json").read_text())
    programs = {kind: manifest["programs"][kind]["image_id"] for kind in ("allowlist", "threshold")}
    if any(not isinstance(pid, list) or len(pid) != 8
           or any(type(word) is not int or not 0 <= word < 2**32 for word in pid)
           for pid in programs.values()):
        raise ValueError("The bundled program manifest is invalid.")
    config = {
        "sequencers": [{"sequencer_addr": "https://testnet.lez.logos.co/"}],
        "seq_poll_timeout": "500ms", "seq_tx_poll_max_blocks": 180,
        "seq_poll_max_retries": 5, "seq_block_poll_max_amount": 100,
        "multi_sequencer_client_config": {"distribution_limit": 1, "calibration_limit": 2},
    }
    os.mkdir(destination, mode=0o700)
    os.chmod(destination, 0o700)
    files = {
        ".commons-readonly": b"Public reads only. No signing keys or write authority.\n",
        ".commons-logos-testnet-wallet": b"Commons testnet reader. No real-value wallet.\n",
        "config.json": (json.dumps(config, indent=2) + "\n").encode(),
        "programs.json": (json.dumps(programs, indent=2) + "\n").encode(),
    }
    for name, data in files.items():
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
        with os.fdopen(os.open(destination / name, flags, 0o600), "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
    if (destination / "storage.json").exists():
        raise RuntimeError("Unexpected private storage in a key-free reader.")
    if stat.S_IMODE(destination.stat().st_mode) != 0o700:
        raise RuntimeError("Reader folder permissions are not owner-only.")
    print(f"Created key-free testnet reader: {destination}")
    print("In Basecamp, open Commons > Connection settings, select the matching")
    print("commons-logos-cli executable and this reader folder, then Connect client.")
    print("Use My workspaces to create your own identity or import an invitation.")
    print("No keys, faucet request, funding, network connection, or transaction were created.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError) as error:
        print(f"Reader setup failed: {error}. Existing files were not overwritten.", file=sys.stderr)
        raise SystemExit(1)
