import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('local_demo',ROOT/'scripts/demo-local.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class ReportTests(unittest.TestCase):
    def test_no_file_means_no_success(self):
        with tempfile.TemporaryDirectory() as d:self.assertEqual(m.read_public_events(Path(d)/'absent'),[])
    def test_only_public_fields_are_read(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'events';p.write_text(json.dumps({'stage':'approve','status':'confirmed','block_id':3,'private_key':'never output','witness':'never output'})+'\n')
            self.assertEqual(m.read_public_events(p),[{'stage':'approve','status':'confirmed','block_id':3}])
    def test_blank_lines_ignored(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'events';p.write_text('\n');self.assertEqual(m.read_public_events(p),[])
    def test_matrix_covers_both_full_flows(self):
        text=(ROOT/'.github/workflows/real-proofs.yml').read_text()
        self.assertIn('["threshold","allowlist-smoke"]',text)
        self.assertIn('DEMO_MODE: ${{ matrix.mode }}',text)
        self.assertIn('fail-fast: false',text)
    def test_success_requirements_not_removed(self):
        text=(ROOT/'scripts/demo-local.py').read_text()
        self.assertIn('len(confirmed) < required',text)
        self.assertIn("r.get('parameter_value') == 42",text)
        self.assertIn("'RISC0_DEV_MODE': '0'",text)
if __name__=='__main__':unittest.main()
