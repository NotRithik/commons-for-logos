"""Unit tests for orchestration/secret filtering, NOT proof or network evidence."""
import importlib.util
import os
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('local_runner', ROOT / 'scripts/demo-local.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)

class LocalRunnerTests(unittest.TestCase):
    def environment(self, extra=None):
        source = {'PATH': '/usr/bin:/bin', 'BONSAI_API_KEY': 'SYNTHETIC', 'BONSAI_API_URL': 'https://invalid.example',
                  'OPENAI_API_KEY': 'SYNTHETIC', 'GITHUB_TOKEN': 'SYNTHETIC', 'RISC0_DEV_MODE': '1', 'RISC0_PROVER': 'bonsai'}
        source.update(extra or {})
        paths = {'lbc_root': '/owned/circuits', 'rapidsnark_lib': '/owned/lib', 'r0vm': '/owned/r0vm'}
        with patch.dict(os.environ, source, clear=True):
            return runner.child_env(ROOT / 'out', ROOT / 'out/run', paths)

    def test_never_inherits_credentials(self):
        env = self.environment()
        self.assertFalse(any(key in env for key in ['BONSAI_API_KEY', 'BONSAI_API_URL', 'OPENAI_API_KEY', 'GITHUB_TOKEN']))

    def test_real_local_proof_settings_are_forced(self):
        env = self.environment()
        self.assertEqual(env['RISC0_DEV_MODE'], '0')
        self.assertEqual(env['RISC0_PROVER'], 'ipc')
        self.assertEqual(env['RISC0_EXECUTOR'], 'ipc')
        self.assertEqual(env['RISC0_SERVER_PATH'], '/owned/r0vm')

    def test_home_and_temporary_files_are_in_run_directory(self):
        env = self.environment()
        self.assertTrue(Path(env['HOME']).is_relative_to(ROOT / 'out/run'))
        self.assertTrue(Path(env['TMPDIR']).is_relative_to(ROOT / 'out/run'))

    def test_public_evidence_excludes_private_witnesses_and_keys(self):
        forbidden = {'witness', 'nullifier_secret_key', 'viewing_private_key', 'mnemonic', 'wallet', 'storage', 'journal', 'input', 'private_key', 'public_accounts'}
        self.assertFalse(forbidden & runner.PUBLIC_FIELDS)
        self.assertTrue({'tx_hash', 'block_id', 'guest_user_cycles', 'status'} <= runner.PUBLIC_FIELDS)

    def test_endpoint_is_loopback_and_matches_checked_in_cli(self):
        self.assertEqual(runner.URL, 'http://127.0.0.1:34341')
        self.assertNotIn('testnet.lez.logos.co', runner.URL)

    def test_expected_protocol_fingerprint_has_eight_words(self):
        self.assertEqual(len(runner.PROTOCOL), 8)
        self.assertTrue(all(isinstance(word, int) and 0 <= word < 2**32 for word in runner.PROTOCOL))
        self.assertEqual(len(runner.PIN), 40)

    def test_stop_none_is_harmless(self):
        runner.stop(None)

if __name__ == '__main__':
    unittest.main()
