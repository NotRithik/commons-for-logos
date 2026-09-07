import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('release_download',ROOT/'scripts/fetch-release-programs.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)

class ReleaseDownloadTests(unittest.TestCase):
    def item(self):
        data=b'public fixture'
        return data,{'file':'commons_allowlist','bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
    def test_unsafe_names_rejected(self):
        _,item=self.item();item['file']='../storage.json'
        with self.assertRaises(ValueError):module.checked_item('allowlist',item)
    def test_digest_and_size_are_required(self):
        data,item=self.item();module.check_bytes(data,item)
        with self.assertRaises(ValueError):module.check_bytes(data+b'x',item)
        with self.assertRaises(ValueError):module.check_bytes(b'x'*len(data),item)
    def test_existing_valid_file_needs_no_network(self):
        data,item=self.item()
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);(p/item['file']).write_bytes(data)
            with patch.object(module.urllib.request,'urlopen',side_effect=AssertionError('network not needed')):
                module.fetch(item,p)
    def test_existing_mismatch_is_not_overwritten(self):
        _,item=self.item()
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);f=p/item['file'];f.write_bytes(b'keep this')
            with self.assertRaises(ValueError):module.fetch(item,p)
            self.assertEqual(f.read_bytes(),b'keep this')
    def test_symlink_is_rejected(self):
        data,item=self.item()
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);target=p/'target';target.write_bytes(data);(p/item['file']).symlink_to(target)
            with self.assertRaises(ValueError):module.fetch(item,p)
    def test_overlarge_artifact_pin_is_rejected(self):
        _,item=self.item();item['bytes']=10**9
        with self.assertRaises(ValueError):module.checked_item('allowlist',item)

if __name__=='__main__':unittest.main()
