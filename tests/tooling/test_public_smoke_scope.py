import importlib.util
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('public_demo_scope',ROOT/'scripts/demo-public.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class PublicScopeTests(unittest.TestCase):
    def test_ci_names_its_smaller_scope_honestly(self):
        text=(ROOT/'.github/workflows/public-testnet.yml').read_text()
        self.assertIn('--claims 1',text);self.assertIn('Real public claim smoke',text);self.assertNotIn('Verify ten',text)
    def test_full_run_default_still_ten(self):
        text=(ROOT/'scripts/demo-public.py').read_text()
        self.assertIn("'--claims', type=int, default=10",text)
        self.assertIn('validate_claim_records(records, args.claims)',text)
        self.assertIn("'required_claims_for_this_run': args.claims",text)
    def test_success_requires_independent_rpc_lookup(self):
        text=(ROOT/'scripts/demo-public.py').read_text()
        self.assertIn("rpc('getTransaction'",text)
        self.assertIn("result[1] != claim['block_id']",text)
class ClaimRecordTests(unittest.TestCase):
    def events(self,count):
        claims=[{'stage':f'claim_distribution_a_{i}','private':True,'status':'confirmed','tx_hash':str(i)} for i in range(count)]
        return claims+[{'stage':'claims_complete' if count==10 else 'claims_partial','unique_private_claims':count,'required_for_this_mode':10}]
    def test_one_claim_only_passes_smoke_requirement(self):
        self.assertEqual(len(m.validate_claim_records(self.events(1),1)),1)
        with self.assertRaises(RuntimeError):m.validate_claim_records(self.events(1),10)
    def test_full_requires_all_ten(self):self.assertEqual(len(m.validate_claim_records(self.events(10),10)),10)
    def test_duplicate_hash_cannot_inflate_count(self):
        events=self.events(10);events[1]['tx_hash']=events[0]['tx_hash']
        with self.assertRaises(RuntimeError):m.validate_claim_records(events,10)
    def test_missing_final_state_event_rejected(self):
        with self.assertRaises(RuntimeError):m.validate_claim_records(self.events(1)[:-1],1)
    def test_public_or_unconfirmed_event_not_counted(self):
        events=self.events(1);events[0]['private']=False
        with self.assertRaises(RuntimeError):m.validate_claim_records(events,1)
    def test_invalid_requested_count_rejected(self):
        for count in [0,11,True]:
            with self.assertRaises(ValueError):m.validate_claim_records([],count)
if __name__=='__main__':unittest.main()
