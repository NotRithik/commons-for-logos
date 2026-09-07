#!/usr/bin/env python3
"""Verify ten fresh allowlist claims against the published testnet release.

This is an explicit acceptance test, not an automatic or scheduled task. It
creates its own disposable test identities; it never imports an existing wallet.
Only public transaction references and counts leave the private run directory.
"""
from __future__ import annotations
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
URL = 'https://testnet.lez.logos.co/'
PIN = '47eba256479f6f785acbd138834340703cd03401'
PROTOCOL = [1334328888, 3910590567, 1244219104, 3671232111, 3138827701, 405554639, 4064616947, 1864368340]
PUBLIC_FIELDS = frozenset(['stage', 'status', 'private', 'tx_hash', 'block_id', 'seconds',
                          'RISC0_DEV_MODE', 'mode', 'distributions', 'unique_private_claims',
                          'required_for_this_mode', 'network', 'guest_user_cycles', 'proof'])

def release_programs(manifest_path: Path, artifact_dir: Path) -> dict:
    manifest = json.loads(manifest_path.read_text())
    if manifest.get('lez_revision') != PIN or manifest.get('testnet_endpoint') != URL:
        raise ValueError('Release targets a different network or protocol revision')
    programs = manifest.get('programs', {})
    for family in ('allowlist', 'threshold'):
        item = programs.get(family, {})
        if item.get('file') != 'commons_' + family:
            raise ValueError('Unexpected release filename')
        image = item.get('image_id')
        if not isinstance(image, list) or len(image) != 8 or any(type(x) is not int or not 0 <= x < 2**32 for x in image):
            raise ValueError('Invalid release image ID')
        path = artifact_dir / item['file']
        if path.is_symlink() or not path.is_file():
            raise ValueError('Release artifact must be a regular file')
        data = path.read_bytes()
        if len(data) != item.get('bytes') or hashlib.sha256(data).hexdigest() != item.get('sha256'):
            raise ValueError('Rebuilt guest does not match the deployed release; refusing transactions')
    return programs

def rpc(method: str, params: list) -> object:
    body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params}).encode()
    req = urllib.request.Request(URL, data=body, headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=30) as response:
        data = json.load(response)
    if 'error' in data:
        raise RuntimeError('Official testnet RPC returned an error for ' + method)
    return data.get('result')

def child_env(run: Path, paths: dict) -> dict:
    env = {key: os.environ[key] for key in ['PATH', 'LANG', 'SSL_CERT_FILE'] if key in os.environ}
    env.update({'HOME': str(run / 'home'), 'TMPDIR': str(run / 'tmp') + '/',
                'RISC0_DEV_MODE': '0', 'RISC0_PROVER': 'ipc', 'RISC0_EXECUTOR': 'ipc',
                'RISC0_SERVER_PATH': paths['r0vm'], 'RAYON_NUM_THREADS': '3',
                'LBC_ROOT_DIR': paths['lbc_root'], 'RAPIDSNARK_LIB_DIR': paths['rapidsnark_lib'],
                'DYLD_LIBRARY_PATH': paths['rapidsnark_lib'], 'LD_LIBRARY_PATH': paths['rapidsnark_lib'],
                'COMMONS_ALLOW_PUBLIC_TESTNET': '1', 'SUPPRESS_VERBOSE_PRINTS': '1',
                'RUST_LOG': 'warn', 'CARGO_NET_OFFLINE': 'true'})
    return env

def public_events(wallet: Path) -> list[dict]:
    path = wallet / 'evidence.jsonl'
    if not path.is_file():
        return []
    output = []
    for line in path.read_text().splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if not isinstance(row, dict):
            continue
        safe = {key: value for key, value in row.items() if key in PUBLIC_FIELDS}
        if 'distributions' in safe:
            safe['distributions'] = [{k: v for k, v in d.items() if k in ['name', 'state_account', 'unique_claims']}
                                     for d in safe['distributions'] if isinstance(d, dict)]
        if safe:
            output.append(safe)
    return output

def stop(process: subprocess.Popen | None) -> None:
    if process is None or process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=15)
    except ProcessLookupError:
        pass

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--confirm-public-testnet', action='store_true', required=True)
    parser.add_argument('--out', type=Path, default=ROOT / 'out')
    parser.add_argument('--published-programs', type=Path, help='Explicit verified release-program directory; otherwise use this build')
    parser.add_argument('--timeout-seconds', type=int, default=4 * 3600)
    args = parser.parse_args()
    out = args.out.resolve()
    if out == ROOT or not out.is_relative_to(ROOT):
        parser.error('output must stay inside this checkout')
    if not 60 <= args.timeout_seconds <= 5 * 3600:
        parser.error('timeout must be 60 seconds to 5 hours')
    artifact_dir = args.published_programs.resolve() if args.published_programs else out / 'artifacts'
    if not artifact_dir.is_relative_to(out):
        parser.error('program artifacts must stay inside the output directory')
    programs = release_programs(ROOT / 'release/manifest.json', artifact_dir)
    print('Using checksum-verified ' + ('published' if args.published_programs else 'rebuilt') + ' program files.', flush=True)
    paths = json.loads((out / 'proof-deps/paths.json').read_text())
    driver = out / 'clients/debug/commons-logos-e2e'
    for binary in [driver, Path(paths['r0vm'])]:
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise RuntimeError('Missing executable prerequisite')
    if rpc('getProgramIds', []).get('privacy_preserving_circuit') != PROTOCOL:
        raise RuntimeError('Public testnet protocol fingerprint changed')
    os.umask(0o077)
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    run = out / 'public-runs' / (stamp + '-' + str(os.getpid()))
    for name in ['home', 'tmp', 'logs', 'wallet', 'public']:
        (run / name).mkdir(parents=True, exist_ok=False)
    wallet = run / 'wallet'
    env = child_env(run, paths)
    deadline = time.monotonic() + args.timeout_seconds
    process = None
    state = 'failed'
    checked = []
    records = []
    def stage(label: str, *arguments: str) -> None:
        nonlocal process
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError('Public acceptance test deadline reached')
        print('Running: ' + label, flush=True)
        with (run / 'logs' / (label + '.log')).open('w') as log:
            process = subprocess.Popen([str(driver), *arguments], cwd=ROOT, env=env,
                                       stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            rc = process.wait(timeout=remaining)
            if rc:
                raise RuntimeError(label + ' failed; private diagnostics retained only in run directory')
        process = None
    try:
        stage('prepare-fresh-test-identities', 'prepare', str(wallet), URL)
        (wallet / 'programs.json').write_text(json.dumps({k: v['image_id'] for k, v in programs.items()}) + '\n')
        stage('ten-real-allowlist-claims', 'claims-a', str(wallet), str(artifact_dir / 'commons_allowlist'), '10')
        records = public_events(wallet)
        claims = [row for row in records if row.get('status') == 'confirmed' and row.get('private') is True
                  and str(row.get('stage', '')).startswith('claim_distribution_a_')]
        if len({row.get('tx_hash') for row in claims}) != 10:
            raise RuntimeError('Expected ten distinct confirmed private transactions')
        if not any(row.get('stage') == 'claims_complete' and row.get('unique_private_claims') == 10 for row in records):
            raise RuntimeError('Final on-chain claim count was not verified')
        for claim in claims:
            result = rpc('getTransaction', [claim['tx_hash']])
            if not isinstance(result, list) or len(result) != 2 or result[1] != claim['block_id']:
                raise RuntimeError('Independent public transaction lookup did not match its receipt')
            checked.append({'tx_hash': claim['tx_hash'], 'block_id': result[1]})
        state = 'passed'
    finally:
        stop(process)
        records = public_events(wallet)
        report = {'status': state, 'network': URL, 'lez_revision': PIN, 'risc0_dev_mode': '0',
                  'prover': 'local-ipc', 'guest_source': 'published-release' if args.published_programs else 'rebuilt', 'identity_source': 'fresh test identities created within this run',
                  'human_adoption_claimed': False, 'programs': {k: v['image_id'] for k, v in programs.items()},
                  'events': records, 'independently_checked_transactions': checked}
        text = json.dumps(report, indent=2) + '\n'
        (run / 'public/report.json').write_text(text)
        (out / 'latest-public-report.json').write_text(text)
        print('Public report: ' + str(run / 'public/report.json'), flush=True)
    print('PASS: ten distinct private claims verified on official testnet.', flush=True)

if __name__ == '__main__':
    main()
