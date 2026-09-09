"""Submission criteria describe completed actions, not one hardcoded demo value."""
import importlib.util
from pathlib import Path
import unittest
p=Path(__file__).resolve().parents[2]/'scripts/check-public-state.py'
s=importlib.util.spec_from_file_location('current_criteria',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
class CurrentCriteriaTests(unittest.TestCase):
    def group(self,value=43):
        return {'threshold':{'state':{'value':value,'threshold':2,'sequence':2,'proposal':{'next_value':value,'executed':True,'unique_approvals':2,'sequence':2}}}}
    def test_completed_authorized_parameter_change_is_not_limited_to_42(self):
        for value in [0,42,43,18446744073709551615]:self.assertTrue(m.check_ready(self.group(value),'LP-0002'))
    def test_pending_mismatched_or_under_threshold_does_not_pass(self):
        for key,value in [('executed',False),('unique_approvals',1),('next_value',42),('sequence',1)]:
            r=self.group();r['threshold']['state']['proposal'][key]=value
            self.assertFalse(m.check_ready(r,'LP-0002'))
    def test_repeated_distribution_is_not_counted_twice(self):
        a={'address':'a','state':{'unique_claims':10}};b={'address':'b','state':{'unique_claims':0}}
        self.assertFalse(m.check_ready({'distributions':[a,a,b]},'LP-0003'))
        b['state']['unique_claims']=10;self.assertTrue(m.check_ready({'distributions':[a,b]},'LP-0003'))
if __name__=='__main__':unittest.main()
