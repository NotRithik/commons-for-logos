#!/usr/bin/env python3
"""Open Commons in a separate Basecamp profile using explicit local proof dependencies.

No funding, wallet import, claim, vote, program deployment or package download occurs.
Install the matching Commons LGX through this profile's Package Manager after launch.
"""
from __future__ import annotations

import argparse
import fcntl
import json
import os
from pathlib import Path
import platform
import plistlib
import stat
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
MARKER = '.commons-launcher-v1'


def regular(path: Path, executable: bool = False) -> Path:
    path = path.expanduser().absolute()
    meta = path.lstat()
    if not stat.S_ISREG(meta.st_mode) or path.is_symlink():
        raise ValueError(f'Expected a regular file: {path}')
    if executable and not os.access(path, os.X_OK):
        raise ValueError(f'File is not executable: {path}')
    return path.resolve()


def basecamp_command(path: Path) -> tuple[Path, Path | None]:
    path = path.expanduser().absolute()
    if path.suffix == '.app' and path.is_dir():
        contents = path / 'Contents'
        info = plistlib.loads(regular(contents / 'Info.plist').read_bytes())
        name = info.get('CFBundleExecutable')
        if not isinstance(name, str) or '/' in name or name in ('', '.', '..'):
            raise ValueError('Unexpected Basecamp bundle executable')
        return regular(contents / 'MacOS' / name, True), contents
    return regular(path, True), None


def private_directory(path: Path, create: bool = False) -> None:
    if create:
        path.mkdir(mode=0o700)
    meta = path.lstat()
    if not stat.S_ISDIR(meta.st_mode) or path.is_symlink() or stat.S_IMODE(meta.st_mode) & 0o077:
        raise ValueError(f'Use an owner-only regular folder (mode 700): {path}')
    if hasattr(os, 'getuid') and meta.st_uid != os.getuid():
        raise ValueError('The profile must belong to the current operating-system user')


def write_new(path: Path, content: str) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, 'O_NOFOLLOW', 0)
    with os.fdopen(os.open(path, flags, 0o600), 'w', encoding='utf-8') as stream:
        stream.write(content)
        stream.flush()
        os.fsync(stream.fileno())


def prepare_profile(root: Path) -> None:
    if root.exists() or root.is_symlink():
        private_directory(root)
        if regular(root / MARKER).read_text().strip() != 'Commons isolated Basecamp profile v1':
            raise ValueError('Refusing to reuse an unrelated existing directory')
    else:
        if not root.parent.is_dir():
            raise ValueError('Create the parent folder first; the launcher will not make unrelated ancestors')
        private_directory(root, create=True)
        write_new(root / MARKER, 'Commons isolated Basecamp profile v1\n')
    for name in ('home', 'basecamp', 'members', 'logs'):
        folder = root / name
        private_directory(folder, create=not folder.exists())
    reader = root / 'reader'
    if not reader.exists():
        subprocess.run([sys.executable, str(ROOT / 'scripts/create-reader.py'), str(reader)], check=True)
    else:
        private_directory(reader)
        for name in ('.commons-readonly', '.commons-logos-testnet-wallet', 'config.json', 'programs.json'):
            regular(reader / name)
        if (reader / 'storage.json').exists():
            raise ValueError('The safe starting reader unexpectedly contains wallet storage; nothing was replaced')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--basecamp', type=Path, required=True, help='Basecamp .app on macOS or executable on Linux')
    parser.add_argument('--client', type=Path, required=True, help='Verified commons-logos-cli with artifacts folder beside it')
    parser.add_argument('--proof-paths', type=Path, required=True, help='paths.json from scripts/fetch-proof-deps.py')
    parser.add_argument('--profile', type=Path, required=True, help='New dedicated directory, or this launcher\'s existing profile')
    parser.add_argument('--threads', type=int, default=3)
    parser.add_argument('--prepare-only', action='store_true', help='Validate dependencies and prepare a key-free profile; do not open the app')
    args = parser.parse_args()
    if platform.system() not in ('Darwin', 'Linux'):
        parser.error('This launcher supports macOS and Linux; use the documented WSL/build workflow elsewhere')
    if not 1 <= args.threads <= 16:
        parser.error('Choose between 1 and 16 local proof threads')
    binary, contents = basecamp_command(args.basecamp)
    client = regular(args.client, True)
    paths = json.loads(regular(args.proof_paths).read_text())
    if paths.get('schema_version') != 1 or paths.get('lez_revision') != '47eba256479f6f785acbd138834340703cd03401':
        raise ValueError('Proof dependencies do not match the pinned Logos release')
    prover = regular(Path(paths['r0vm']), True)
    circuits = Path(paths['lbc_root']).resolve(strict=True)
    snark = Path(paths['rapidsnark_lib']).resolve(strict=True)
    if not circuits.is_dir() or not snark.is_dir():
        raise ValueError('Circuit and local proof-library folders must exist')
    for family in ('threshold', 'allowlist'):
        regular(client.parent / 'artifacts' / ('commons_' + family))
    profile = args.profile.expanduser().absolute()
    if any(parent.is_symlink() for parent in profile.parents):
        raise ValueError('Choose a profile without symbolic-link parents')
    prepare_profile(profile)
    if args.prepare_only:
        print('READY: isolated key-free profile and local proof paths checked. No app or transaction started.')
        return 0
    # Hold a kernel-backed lease for this launcher and its child lifetime.
    # A crash releases it; a persistent filename is not a stale lock to delete.
    flags = os.O_RDWR | os.O_CREAT | getattr(os, 'O_NOFOLLOW', 0)
    lock_fd = os.open(profile / '.basecamp-launch.lock', flags, 0o600)
    try:
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(lock_fd)
        print('This Commons profile already has a running launcher. No second app was started.')
        return 0
    # Refuse a second host for the same dedicated data directory, without stopping it.
    if hasattr(os, 'getuid'):
        processes = subprocess.check_output(['ps', '-axo', 'args='], text=True)
        for line in processes.splitlines():
            if str(binary) in line and '--user-dir ' + str(profile / 'basecamp') in line:
                print('This Commons profile is already open. No second app was started.')
                return 0
    env = {key: os.environ[key] for key in ('PATH', 'LANG', 'LC_ALL', 'TMPDIR', 'DISPLAY',
            'WAYLAND_DISPLAY', 'XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS', 'SSL_CERT_FILE') if key in os.environ}
    env.update({'HOME': str(profile / 'home'), 'LOGOS_USER_DIR': str(profile / 'basecamp'),
                'COMMONS_DEFAULT_CLI': str(client), 'COMMONS_DEFAULT_WALLET': str(profile / 'reader'),
                'COMMONS_GOVERNANCE_HOME': str(profile / 'members'), 'COMMONS_ALLOW_PUBLIC_TESTNET': '1',
                'RISC0_DEV_MODE': '0', 'RISC0_PROVER': 'ipc', 'RISC0_EXECUTOR': 'ipc',
                'RISC0_SERVER_PATH': str(prover), 'LBC_ROOT_DIR': str(circuits),
                'RAPIDSNARK_LIB_DIR': str(snark), 'RAYON_NUM_THREADS': str(args.threads),
                'SUPPRESS_VERBOSE_PRINTS': '1'})
    env['DYLD_LIBRARY_PATH' if platform.system() == 'Darwin' else 'LD_LIBRARY_PATH'] = str(snark)
    if contents is not None:
        env['QT_PLUGIN_PATH'] = str(contents / 'Resources/qt/plugins')
        env['QML2_IMPORT_PATH'] = str(contents / 'Resources/qt/qml')
    log = profile / 'logs' / f'basecamp-{time.time_ns()}.log'
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, 'O_NOFOLLOW', 0)
    with os.fdopen(os.open(log, flags, 0o600), 'wb') as stream:
        process = subprocess.Popen([str(binary), '--user-dir', str(profile / 'basecamp')], env=env,
                                   cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
        print('Opened a separate Commons Basecamp profile. RISC0_DEV_MODE=0; proof generation is local.', flush=True)
        print('Install the verified Commons LGX in this window\'s Package Manager, then open Commons.', flush=True)
        print('The starting workspace is view-only. Use My workspaces to select or create your own identity.', flush=True)
        print('Keep this terminal and the app open during a private proof. Local logs are not public evidence.', flush=True)
        return process.wait()


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f'Commons launch did not finish: {error}', file=sys.stderr)
        raise SystemExit(1)
