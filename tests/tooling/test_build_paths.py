import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('build_rust', ROOT / 'scripts/build-rust.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class BuildPathTests(unittest.TestCase):
    def test_paths_with_spaces_are_separate_flags_not_shell_fragments(self):
        flags = module.remap_flags(Path('/tmp/example project'), Path('/tmp/private cargo'), Path('/tmp/private rust'))
        self.assertEqual(len(flags), 3)
        self.assertEqual(flags[0], f'--remap-path-prefix={Path("/tmp/example project").resolve()}=/commons')
        self.assertEqual(len('\x1f'.join(flags).split('\x1f')), 3)

    def test_local_runner_uses_the_remapped_client_build(self):
        source = (ROOT / 'scripts/demo-local.py').read_text()
        self.assertIn("out / 'clients/debug/commons-logos-e2e'", source)
        self.assertIn("out / 'host/debug/sequencer_service'", source)

    def test_guest_keeps_required_risc0_target_flags(self):
        flags = module.flags('guest', Path('/tmp/repo'), Path('/tmp/cargo'), Path('/tmp/rust'))
        for item in ['getrandom_backend="custom"', 'passes=lower-atomic', 'link-arg=-Ttext=0x00200800', 'panic=abort', 'debuginfo=0']:
            self.assertIn(item, flags)

    def test_host_does_not_receive_guest_linker_or_randomness_flags(self):
        flags = module.flags('host', Path('/tmp/repo'), Path('/tmp/cargo'), Path('/tmp/rust'))
        self.assertTrue(all(f.startswith('--remap-path-prefix=') for f in flags))

if __name__ == '__main__':
    unittest.main()
