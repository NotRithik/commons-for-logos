import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('public_runner', ROOT / 'scripts/demo-public.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class PublicRunnerTests(unittest.TestCase):
    def make_release(self, path):
        programs = {}
        for family in ('allowlist', 'threshold'):
            data = ('fixture-' + family).encode()
            (path / ('commons_' + family)).write_bytes(data)
            programs[family] = {'file': 'commons_' + family, 'bytes': len(data),
                                'sha256': hashlib.sha256(data).hexdigest(), 'image_id': [1]*8}
        manifest = {'lez_revision': module.PIN, 'testnet_endpoint': module.URL, 'programs': programs}
        file = path / 'manifest.json'
        file.write_text(json.dumps(manifest))
        return file, manifest

    def test_valid_artifacts_are_checked_before_network_actions(self):
        with tempfile.TemporaryDirectory() as d:
            file, manifest = self.make_release(Path(d))
            self.assertEqual(module.release_programs(file, Path(d)), manifest['programs'])

    def test_mismatched_artifact_fails_closed(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d); file, _ = self.make_release(p)
            (p / 'commons_allowlist').write_bytes(b'changed bytes')
            with self.assertRaisesRegex(ValueError, 'does not match'):
                module.release_programs(file, p)

    def test_invalid_network_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d); file, manifest = self.make_release(p)
            manifest['testnet_endpoint'] = 'https://another-host.invalid/'
            file.write_text(json.dumps(manifest))
            with self.assertRaises(ValueError): module.release_programs(file, p)

    def test_boolean_is_not_a_program_id_word(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d); file, manifest = self.make_release(p)
            manifest['programs']['allowlist']['image_id'][0] = True
            file.write_text(json.dumps(manifest))
            with self.assertRaises(ValueError): module.release_programs(file, p)

    def test_artifact_symlink_is_not_accepted(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d); file, _ = self.make_release(p)
            target = p / 'original'
            (p / 'commons_allowlist').rename(target)
            (p / 'commons_allowlist').symlink_to(target)
            with self.assertRaises(ValueError): module.release_programs(file, p)

    def test_environment_does_not_forward_api_keys_or_hosted_prover(self):
        paths = {'r0vm': '/reviewed/r0vm', 'lbc_root': '/reviewed/circuits', 'rapidsnark_lib': '/reviewed/lib'}
        with patch.dict(os.environ, {'BONSAI_API_KEY':'fixture-secret', 'OPENAI_API_KEY':'fixture-secret', 'RISC0_PROVER':'bonsai'}):
            env = module.child_env(Path('/test/run'), paths)
        self.assertNotIn('BONSAI_API_KEY', env)
        self.assertNotIn('OPENAI_API_KEY', env)
        self.assertEqual(env['RISC0_PROVER'], 'ipc')
        self.assertEqual(env['RISC0_DEV_MODE'], '0')
        self.assertEqual(env['HOME'], '/test/run/home')
        self.assertEqual(env['RISC0_SERVER_PATH'], '/reviewed/r0vm')

    def test_public_report_omits_secret_and_path_fields(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)
            row = {'stage':'confirmed', 'tx_hash':'a'*64, 'witness':'private', 'wallet_path':'private',
                   'distributions':[{'name':'distribution_a','state_account':'a'*64,'unique_claims':10,'secret':'private'}]}
            (p / 'evidence.jsonl').write_text(json.dumps(row)+'\n')
            text = json.dumps(module.public_events(p))
            self.assertNotIn('private', text)
            self.assertIn('unique_claims', text)

    def test_missing_report_is_empty_not_success(self):
        with tempfile.TemporaryDirectory() as d: self.assertEqual(module.public_events(Path(d)), [])

    def test_prepare_matches_the_pinned_cli_endpoint_spelling(self):
        self.assertEqual(module.prepare_arguments(Path('/isolated/wallet')),
                         ('prepare', '/isolated/wallet', 'https://testnet.lez.logos.co'))

    def test_error_categories_do_not_publish_private_diagnostics(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'private.log'
            p.write_text('Error: unsupported endpoint\nprivate secret material never printed')
            result=module.failure_codes(p)
            self.assertEqual(result,['ENDPOINT_ARGUMENT_REJECTED'])
            self.assertNotIn('secret',str(result))

    def test_no_stop_action_without_process(self):
        module.stop(None)

if __name__ == '__main__':
    unittest.main()
