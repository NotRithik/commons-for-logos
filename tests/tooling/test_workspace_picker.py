from pathlib import Path
import re
import unittest
ROOT=Path(__file__).resolve().parents[2]
QML=(ROOT/'module/src/qml/Main.qml').read_text()
PICKER=(ROOT/'module/src/qml/WorkspacePicker.qml').read_text()
class WorkspacePickerTests(unittest.TestCase):
    def test_saved_workspace_uses_in_scene_bounded_picker(self):
        self.assertRegex(QML,r'WorkspacePicker\s*\{\s*id: workspacePicker')
        self.assertIn('viewportItem: root',QML)
        self.assertIn('popupType: Basic.Popup.Item',PICKER)
        self.assertIn('maximumPopupHeight: 328',PICKER)
        self.assertIn('roomBelow',PICKER)
        self.assertIn('roomAbove',PICKER)
    def test_rows_and_popup_inherit_palette_and_font(self):
        self.assertIn('font: control.font',PICKER)
        self.assertIn('textFormat: Text.PlainText',PICKER)
        self.assertIn('color: control.palette.window',PICKER)
        self.assertIn('Basic.ScrollBar.vertical:',PICKER)
        self.assertIn('clip: true',PICKER)
        self.assertNotRegex(PICKER,r'#[0-9a-fA-F]{6}')
    def test_workspace_selection_still_only_opens_saved_profile(self):
        code=QML[QML.index('id: workspacePicker'):]
        handler=re.search(r'onActivated: ([^\n]+)',code).group(1)
        self.assertEqual(handler,'root.callBackend(root.backend.openSavedProfile(currentIndex))')
    def test_package_contains_the_separate_qml_component(self):
        self.assertIn("'qml/WorkspacePicker.qml'",(ROOT/'scripts/package-lgx.py').read_text())
        self.assertIn("'qml/WorkspaceState.js'",(ROOT/'scripts/package-lgx.py').read_text())
if __name__=='__main__':unittest.main()
