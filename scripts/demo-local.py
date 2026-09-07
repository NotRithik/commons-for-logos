#!/usr/bin/env python3
"""Run real local-sequencer tests; never connects to a public testnet or model API.

Build prerequisites with prepare-local.sh first. Full proofs are CPU-intensive.
The raw transcript and synthetic wallet remain private. Only whitelisted receipt
fields are copied into the public report. No program execution is mocked here.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
PIN = '47eba256479f6f785acbd138834340703cd03401'
PROTOCOL = [1334328888, 3910590567, 1244219104, 3671232111, 3138827701, 405554639, 4064616947, 1864368340]
URL = 'http://127.0.0.1:34341'
PUBLIC_FIELDS = {'stage', 'status', 'RISC0_DEV_MODE', 'private', 'tx_hash', 'block_id', 'seconds', 'program', 'image_id', 'guest_user_cycles', 'proof', 'threshold', 'parameter_value', 'network', 'distributions', 'unique_private_claims'}


def rpc(method: str) -> object:
    body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': []}).encode()
    request = urllib.request.Request(URL, data=body, headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=5) as response:
        result = json.load(response)
    if 'error' in result:
        raise RuntimeError('Local RPC error for ' + method)
    return result['result']


def stop(process: subprocess.Popen | None) -> None:
    if process is None or process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=10)
    except ProcessLookupError:
        pass


def child_env(out: Path, run: Path, paths: dict[str, str]) -> dict[str, str]:
    # Deliberately excludes hosted-prover credentials and unrelated API keys.
    env = {k: os.environ[k] for k in ['PATH', 'LANG', 'SSL_CERT_FILE', 'DYLD_LIBRARY_PATH', 'LD_LIBRARY_PATH'] if k in os.environ}
    env.update({'HOME': str(run / 'home'), 'TMPDIR': str(run / 'tmp') + '/', 'RUST_LOG': 'warn',
                'RISC0_DEV_MODE': '0', 'RISC0_PROVER': 'ipc', 'RISC0_EXECUTOR': 'ipc',
                'RAYON_NUM_THREADS': os.environ.get('COMMONS_PROOF_THREADS', '2'),
                'SUPPRESS_VERBOSE_PRINTS': '1', 'CARGO_NET_OFFLINE': 'true',
                'LBC_ROOT_DIR': paths['lbc_root'], 'RAPIDSNARK_LIB_DIR': paths['rapidsnark_lib'],
                'RISC0_SERVER_PATH': paths['r0vm']})
    lib = paths['rapidsnark_lib']
    for key in ['DYLD_LIBRARY_PATH', 'LD_LIBRARY_PATH']:
        env[key] = lib + (':' + env[key] if env.get(key) else '')
    return env


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode', choices=['threshold', 'allowlist-smoke', 'all'], default='all')
    parser.add_argument('--out', type=Path, default=ROOT / 'out')
    parser.add_argument('--timeout-seconds', type=int, default=5 * 3600)
    args = parser.parse_args()
    out = args.out.resolve()
    if not out.is_relative_to(ROOT) or out == ROOT:
        raise SystemExit('Build output must stay inside this checkout')
    if not 60 <= args.timeout_seconds <= 6 * 3600:
        raise SystemExit('Timeout must be 60 seconds to 6 hours')
    paths = json.loads((out / 'proof-deps/paths.json').read_text())
    driver, node = out / 'host/debug/commons-logos-e2e', out / 'host/debug/sequencer_service'
    for binary in [driver, node, Path(paths['r0vm'])]:
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise SystemExit('Missing executable prerequisite: ' + str(binary))
    actual = subprocess.check_output(['git', '-C', str(out / 'lez'), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != PIN:
        raise SystemExit('Refusing mismatched local sequencer source pin')
    with socket.socket() as check:
        check.settimeout(1)
        if check.connect_ex(('127.0.0.1', 34341)) == 0:
            raise SystemExit('Port 34341 is in use; this script will not stop an existing service')
    os.umask(0o077)
    run = out / ('local-' + time.strftime('%Y%m%d-%H%M%S', time.gmtime()))
    run.mkdir(mode=0o700)
    for folder in ['home', 'tmp', 'wallet', 'node', 'public']:
        (run / folder).mkdir(mode=0o700)
    env = child_env(out, run, paths)
    deadline = time.monotonic() + args.timeout_seconds
    node_process: subprocess.Popen | None = None
    command_process: subprocess.Popen | None = None
    records: list[dict] = []
    stage_log = run / 'private-driver.log'
    state = 'failed'

    def stage(label: str, *parameters: str) -> None:
        nonlocal command_process
        print(json.dumps({'stage': label, 'status': 'started', 'RISC0_DEV_MODE': '0', 'prover': 'local-ipc'}), flush=True)
        with stage_log.open('ab') as log:
            command_process = subprocess.Popen([str(driver), *parameters], cwd=ROOT, env=env,
                stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            previous = 0.0
            while command_process.poll() is None:
                if time.monotonic() >= deadline:
                    stop(command_process)
                    raise TimeoutError('Local proof deadline reached during ' + label)
                if time.monotonic() - previous > 60:
                    print(json.dumps({'stage': label, 'status': 'running', 'real_proofs': True}), flush=True)
                    previous = time.monotonic()
                time.sleep(2)
            if command_process.returncode:
                raise RuntimeError('Local stage failed: ' + label + '; raw output retained privately')
        print(json.dumps({'stage': label, 'status': 'finished'}), flush=True)

    try:
        stage('prepare-fresh-local-wallet', 'prepare-local', str(run / 'wallet'))
        config = json.loads((out / 'lez/lez/sequencer/service/configs/debug/sequencer_config.json').read_text())
        config['home'] = str(run / 'node')
        config['block_create_timeout'] = '1s'
        if 'gossip' in config:
            config['gossip']['listen_addr'] = '/ip4/127.0.0.1/udp/0/quic-v1'
            config['gossip']['bootstrap_peers'] = []
        config['metrics_address'] = None
        plan = json.loads((run / 'wallet/public-plan.json').read_text())
        config['genesis'].append({'supply_account': {'account_id': plan['payer'], 'balance': 10000000000000}})
        config_path = run / 'node-config.json'
        config_path.write_text(json.dumps(config, indent=2))
        with (run / 'private-node.log').open('wb') as log:
            node_process = subprocess.Popen([str(node), str(config_path), '--listen-address', '127.0.0.1', '--port', '34341'],
                cwd=run, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        ready = False
        for _ in range(120):
            if node_process.poll() is not None:
                raise RuntimeError('Local sequencer exited before readiness')
            try:
                if rpc('getProgramIds').get('privacy_preserving_circuit') != PROTOCOL:
                    raise RuntimeError('Unexpected local private-circuit fingerprint')
                ready = True
                break
            except (OSError, ValueError):
                time.sleep(1)
        if not ready:
            raise TimeoutError('Local sequencer did not become ready')
        stage('deploy-real-programs', 'deploy', str(run / 'wallet'), str(out / 'artifacts'))
        if args.mode in ['allowlist-smoke', 'all']:
            stage('real-private-allowlist-claims', 'claims-smoke', str(run / 'wallet'), str(out / 'artifacts/commons_allowlist'))
        if args.mode in ['threshold', 'all']:
            stage('real-private-threshold-lifecycle', 'threshold', str(run / 'wallet'), str(out / 'artifacts/commons_threshold'))
        for line in (run / 'wallet/evidence.jsonl').read_text().splitlines():
            value = json.loads(line)
            record = {k: v for k, v in value.items() if k in PUBLIC_FIELDS}
            if record:
                records.append(record)
        confirmed = [r for r in records if r.get('status') == 'confirmed' and r.get('private') is True]
        required = (2 if args.mode in ['all', 'allowlist-smoke'] else 0) + (3 if args.mode in ['all', 'threshold'] else 0)
        if len(confirmed) < required:
            raise RuntimeError('Not enough confirmed private transactions; refusing to report success')
        if args.mode in ['threshold', 'all'] and not any(r.get('stage') == 'threshold_complete' and r.get('parameter_value') == 42 for r in records):
            raise RuntimeError('Threshold parameter update was not verified')
        state = 'passed'
    finally:
        stop(command_process)
        stop(node_process)
        report = {'status': state, 'mode': args.mode, 'network': 'local-standalone-only',
                  'lez_revision': PIN, 'risc0_dev_mode': '0', 'prover': 'local-ipc', 'events': records}
        (run / 'public/report.json').write_text(json.dumps(report, indent=2) + '\n')
        (out / 'latest-local-report.json').write_text(json.dumps(report, indent=2) + '\n')
        print('Sanitized report: ' + str(run / 'public/report.json'), flush=True)
    print('PASS: real standalone sequencer and real private proofs; not public-testnet evidence.', flush=True)

if __name__ == '__main__':
    main()
