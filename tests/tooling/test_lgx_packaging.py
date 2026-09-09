import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('lgx_packaging',ROOT/'scripts/package-lgx.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)

class LgxPayloadTests(unittest.TestCase):
    def payload(self,p):
        names=['commons_primitives_ui_plugin.dylib','commons_primitives_ui_replica_factory.dylib','qml/Main.qml','qml/WorkspacePicker.qml','qml/WorkspaceState.js','icons/commons.svg']
        for name in names:
            path=p/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text('test fixture')
        (p/'metadata.json').write_text(json.dumps({'name':module.NAME,'type':'ui_qml','view':'qml/Main.qml','version':'0.1.0'}))
    def test_only_explicit_module_files_are_packaged(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);self.payload(p);(p/'storage.json').write_text('never package');(p/'private.key').write_text('never package')
            names,_=module.files_for_variant(p,'darwin-arm64')
            self.assertEqual(len(names),7);self.assertNotIn('storage.json',names);self.assertNotIn('private.key',names)
    def test_missing_plugin_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);self.payload(p);(p/'commons_primitives_ui_plugin.dylib').unlink()
            with self.assertRaises(ValueError):module.files_for_variant(p,'darwin-arm64')
    def test_symlink_payload_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);self.payload(p);q=p/'qml/Main.qml';q.rename(p/'original');q.symlink_to(p/'original')
            with self.assertRaises(ValueError):module.files_for_variant(p,'darwin-arm64')
    def test_other_module_metadata_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);self.payload(p);m=json.loads((p/'metadata.json').read_text());m['name']='another_module';(p/'metadata.json').write_text(json.dumps(m))
            with self.assertRaises(ValueError):module.files_for_variant(p,'darwin-arm64')
    def test_unknown_platform_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaises(ValueError):module.files_for_variant(Path(d),'arbitrary')
    def test_linux_requires_linux_libraries(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d);self.payload(p)
            with self.assertRaises(ValueError):module.files_for_variant(p,'linux-x86_64')
    def test_view_recording_has_stop_and_one_hour_cap(self):
        text=(ROOT/'module/src/qml/Main.qml').read_text()
        self.assertIn('root.captureFrames >= 2400',text)
        self.assertIn('interval: 1500',text)
        self.assertRegex(text, r'onConfiguredChanged:\s*\{[^}]*if \(!configured\) captureRecording = false')
        self.assertIn('const saved = image.saveToFile',text)
        self.assertIn('root.captureFrames += 1',text)
        self.assertIn('root.connectionExpanded = false',text)

if __name__=='__main__':unittest.main()
