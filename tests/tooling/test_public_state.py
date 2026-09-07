import importlib.util
from pathlib import Path
import struct
import unittest
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('public_state',ROOT/'scripts/check-public-state.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)

def distribution(claims=()):
    return b'COMNSD01'+b'x'*32+struct.pack('<II',10,len(claims))+b''.join(claims)
def group(approvals=(),executed=False,value=7,sequence=1):
    return (b'COMNSM01'+b'x'*32+struct.pack('<IIqQBQqI',3,2,value,sequence,1,sequence,42,len(approvals))+b''.join(approvals)+bytes([executed]))

class PublicStateTests(unittest.TestCase):
    def test_distribution_count(self):self.assertEqual(m.decode_distribution(distribution([b'a'*32,b'b'*32]))['unique_claims'],2)
    def test_duplicate_claims_rejected(self):
        with self.assertRaises(m.InvalidState):m.decode_distribution(distribution([b'a'*32,b'a'*32]))
    def test_truncated_state_rejected(self):
        with self.assertRaises(m.InvalidState):m.decode_distribution(distribution()[:-1])
    def test_trailing_state_rejected(self):
        with self.assertRaises(m.InvalidState):m.decode_distribution(distribution()+b'x')
    def test_wrong_magic_rejected(self):
        with self.assertRaises(m.InvalidState):m.decode_distribution(b'WRONG001'+distribution()[8:])
    def test_no_proposal(self):
        raw=b'COMNSM01'+b'x'*32+struct.pack('<IIqQB',3,2,7,0,0)
        self.assertIsNone(m.decode_group(raw)['proposal'])
    def test_partial_approval_preserved(self):self.assertEqual(m.decode_group(group([b'a'*32]))['proposal']['unique_approvals'],1)
    def test_executed_threshold(self):self.assertTrue(m.decode_group(group([b'a'*32,b'b'*32],True,42))['proposal']['executed'])
    def test_early_execution_rejected(self):
        with self.assertRaises(m.InvalidState):m.decode_group(group([b'a'*32],True,42))
    def test_wrong_final_value_rejected(self):
        with self.assertRaises(m.InvalidState):m.decode_group(group([b'a'*32,b'b'*32],True,7))
    def test_duplicate_approval_rejected(self):
        with self.assertRaises(m.InvalidState):m.decode_group(group([b'a'*32,b'a'*32]))
    def test_noncanonical_boolean_rejected(self):
        with self.assertRaises(m.InvalidState):m.decode_group(group()[:-1]+b'\x02')
    def test_known_group_address(self):self.assertEqual(m.account_hex('DdVsxpch4oFvjbEeUyfqePmSjpTbjFunwML266WRyg4U'),'bba5ce32066795d84deebab88fa45e4da20a51088eb6bad3fb83bf237c3bd965')
    def test_zero_address_encoding(self):self.assertEqual(m.account_hex('1'*32),'00'*32)
    def test_invalid_base58_rejected(self):
        with self.assertRaises(ValueError):m.account_hex('0'*32)
    def test_write_rpc_method_forbidden(self):
        with self.assertRaises(ValueError):m.rpc('sendTransaction',[])

if __name__=='__main__':unittest.main()
