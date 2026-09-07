from pathlib import Path
import re
import unittest
ROOT=Path(__file__).resolve().parents[2]
class ThemeTests(unittest.TestCase):
    def test_uses_host_theme_and_controls(self):
        text=(ROOT/'module/src/qml/Main.qml').read_text()
        for token in ['import Logos.Theme','import Logos.Controls','component Action: LogosButton','LogosTabBar','LogosTabButton','Theme.typography.publicSans']:
            self.assertIn(token,text)
    def test_no_duplicated_literal_color_palette(self):
        text=(ROOT/'module/src/qml/Main.qml').read_text()
        self.assertFalse(re.search(r'#[0-9a-fA-F]{6}',text))
    def test_optional_profile_keeps_validation(self):
        text=(ROOT/'module/src/commons_primitives_backend.cpp').read_text()
        self.assertIn('COMMONS_DEFAULT_CLI',text);self.assertIn('configure(defaultCli, defaultWallet)',text)
    def test_no_fonts_added_to_module(self):
        self.assertFalse([p for p in (ROOT/'module').rglob('*') if p.suffix.lower() in ['.ttf','.otf','.woff','.woff2']])
if __name__=='__main__':unittest.main()
