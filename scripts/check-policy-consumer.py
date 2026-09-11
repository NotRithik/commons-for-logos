#!/usr/bin/env python3
"""Independently read the deployed consumer and its bound threshold policy; no wallet."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('commons_public_reader', ROOT / 'scripts/check-public-state.py')
if spec is None or spec.loader is None:
    raise RuntimeError('The matching Commons public-state reader is missing')
reader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reader)


def decode_consumer(data: bytes) -> dict:
    # Borsh: magic[8], program_id[8*u32], account_id[32], 3*i64, sequence:u64.
    if len(data) != 104:
        raise reader.InvalidState('Consumer data must contain exactly 104 bytes')
    d = reader.Decoder(data)
    if d.take(8) != b'COMNSC01':
        raise reader.InvalidState('Wrong consumer state version')
    owner = [d.number('I') for _ in range(8)]
    policy = d.take(32).hex()
    minimum, maximum, value, sequence = d.number('q'), d.number('q'), d.number('q'), d.number('Q')
    d.finish()
    if not minimum <= value <= maximum:
        raise reader.InvalidState('Consumer value is outside its configured bounds')
    return {'policy_program': owner, 'policy_hex': policy, 'minimum': minimum,
            'maximum': maximum, 'value': value, 'last_consumed_sequence': sequence}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, help='Optional new evidence path within this checkout')
    args = parser.parse_args()
    manifest = json.loads((ROOT / 'release/adapter.json').read_text())
    original = json.loads((ROOT / 'release/manifest.json').read_text())
    if manifest.get('schema_version') != 1 or manifest.get('endpoint') != reader.ENDPOINT:
        raise ValueError('Unsupported adapter manifest or endpoint')
    if original['testnet_endpoint'] != reader.ENDPOINT:
        raise ValueError('Original release uses a different endpoint')
    if reader.rpc('getProgramIds', []).get('privacy_preserving_circuit') != reader.PROTOCOL:
        raise RuntimeError('Current testnet protocol differs from this release')
    first = reader.rpc('getLastBlockId', [])
    consumer_address, policy_address = manifest['consumer'], manifest['policy']
    consumer_hex, policy_hex = reader.account_hex(consumer_address), reader.account_hex(policy_address)
    if consumer_hex == policy_hex:
        raise reader.InvalidState('Consumer and policy must be different accounts')
    account = reader.rpc('getAccount', [consumer_address])
    if account['program_owner'] != manifest['program_id']:
        raise reader.InvalidState('Consumer owner differs from the published adapter program')
    state = decode_consumer(bytes(account['data']))
    expected_policy_owner = original['programs']['threshold']['image_id']
    if state['policy_program'] != expected_policy_owner or state['policy_hex'] != policy_hex:
        raise reader.InvalidState('Consumer policy binding differs from the published reference')
    policy_account = reader.rpc('getAccount', [policy_address])
    if policy_account['program_owner'] != expected_policy_owner:
        raise reader.InvalidState('Policy is not owned by the published threshold program')
    policy_state = reader.decode_group(bytes(policy_account['data']))
    proposal = policy_state['proposal']
    final_value_matches = state['value'] == manifest['expected_value']
    final_sequence_matches = state['last_consumed_sequence'] == manifest['last_consumed_sequence']
    live_policy_matches = bool(proposal and proposal['executed'] and
                              proposal['unique_approvals'] >= policy_state['threshold'] and
                              proposal['sequence'] == state['last_consumed_sequence'] and
                              policy_state['value'] == state['value'])
    result = {'checked_at': datetime.now(timezone.utc).isoformat(), 'endpoint': reader.ENDPOINT,
              'observed_block_range': [first, reader.rpc('getLastBlockId', [])],
              'wallet_loaded': False, 'transactions_submitted': 0,
              'consumer': {'address': consumer_address, 'hex': consumer_hex,
                           'program_id': account['program_owner'], 'state': state},
              'policy': {'address': policy_address, 'program_id': policy_account['program_owner'],
                         'state': policy_state},
              'matches_recorded_consumer_value': final_value_matches,
              'matches_recorded_consumed_sequence': final_sequence_matches,
              'matches_current_executed_policy': live_policy_matches,
              'not_proven_by_this_read': ['historical approval privacy', 'replay rejection',
                                          'original transaction timings', 'prize acceptance']}
    result['success'] = final_value_matches and final_sequence_matches and live_policy_matches
    text = json.dumps(result, indent=2) + '\n'
    print(text, end='')
    if args.output is not None:
        target = args.output.resolve()
        if not target.is_relative_to(ROOT):
            raise ValueError('Evidence output must remain inside this checkout')
        target.parent.mkdir(parents=True, exist_ok=True)
        # Evidence snapshots are append-only by filename; never replace an older observation.
        with target.open('x', encoding='utf-8') as output:
            output.write(text)
    if not result['success']:
        raise SystemExit('Current state no longer matches this recorded consumer demonstration')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        print(f'Consumer verification failed: {error}', file=sys.stderr)
        raise SystemExit(1)
