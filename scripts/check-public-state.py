#!/usr/bin/env python3
"""Read and decode the published Commons testnet state without a wallet or prover."""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import struct
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
ENDPOINT = 'https://testnet.lez.logos.co/'
PROTOCOL = [1334328888,3910590567,1244219104,3671232111,3138827701,405554639,4064616947,1864368340]
ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

class InvalidState(ValueError):
    pass

class Decoder:
    def __init__(self, data: bytes):
        self.data, self.at = data, 0
    def take(self, count: int) -> bytes:
        if count < 0 or self.at+count > len(self.data):
            raise InvalidState('Truncated account state')
        value=self.data[self.at:self.at+count];self.at+=count;return value
    def number(self, format: str):
        return struct.unpack('<'+format,self.take(struct.calcsize('<'+format)))[0]
    def boolean(self) -> bool:
        value=self.number('B')
        if value not in (0,1):raise InvalidState('Invalid boolean')
        return bool(value)
    def hashes(self, limit: int) -> list[bytes]:
        count=self.number('I')
        if count>limit:raise InvalidState('Count exceeds membership size')
        values=[self.take(32) for _ in range(count)]
        if len(set(values))!=count:raise InvalidState('Duplicate application nullifier')
        return values
    def finish(self):
        if self.at!=len(self.data):raise InvalidState('Unexpected trailing account data')

def membership_size(value: int):
    if not 1<=value<=256:raise InvalidState('Invalid membership size')

def decode_distribution(data: bytes) -> dict:
    d=Decoder(data)
    if d.take(8)!=b'COMNSD01':raise InvalidState('Wrong distribution magic')
    root=d.take(32).hex();members=d.number('I');membership_size(members)
    claims=d.hashes(members);d.finish()
    return {'root':root,'member_count':members,'unique_claims':len(claims)}

def decode_group(data: bytes) -> dict:
    d=Decoder(data)
    if d.take(8)!=b'COMNSM01':raise InvalidState('Wrong group magic')
    root=d.take(32).hex();members=d.number('I');membership_size(members)
    threshold=d.number('I')
    if not 1<=threshold<=members:raise InvalidState('Invalid approval threshold')
    value=d.number('q');sequence=d.number('Q');has_proposal=d.boolean();proposal=None
    if has_proposal:
        pseq=d.number('Q');next_value=d.number('q');approvals=d.hashes(members);executed=d.boolean()
        if pseq!=sequence:raise InvalidState('Proposal sequence mismatch')
        if executed and (len(approvals)<threshold or value!=next_value):
            raise InvalidState('Executed proposal does not meet threshold/value rules')
        proposal={'sequence':pseq,'next_value':next_value,'unique_approvals':len(approvals),'executed':executed}
    d.finish()
    return {'root':root,'member_count':members,'threshold':threshold,'value':value,'sequence':sequence,'proposal':proposal}

def account_hex(address: str) -> str:
    if not isinstance(address,str) or not 32<=len(address)<=44:raise ValueError('Invalid address length')
    n=0
    for c in address:
        if c not in ALPHABET:raise ValueError('Invalid base58 address')
        n=n*58+ALPHABET.index(c)
    raw=(b'\0'*(len(address)-len(address.lstrip('1'))))+(n.to_bytes((n.bit_length()+7)//8,'big') if n else b'')
    if len(raw)!=32:raise ValueError('Address must contain 32 bytes')
    return raw.hex()

def rpc(method: str, params: list) -> object:
    if method not in ['getProgramIds','getLastBlockId','getAccount','getTransaction']:
        raise ValueError('Read-only RPC method required')
    request=urllib.request.Request(ENDPOINT,data=json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params}).encode(),headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(request,timeout=25) as response:
        if response.geturl()!=ENDPOINT:raise RuntimeError('Unexpected endpoint redirect')
        data=json.load(response)
    if 'error' in data:raise RuntimeError('Public RPC failed for '+method)
    return data['result']

def check_ready(report: dict, prize: str) -> bool:
    if prize=='LP-0002':
        g=report['threshold']['state'];p=g['proposal']
        return bool(p and p['executed'] and p['unique_approvals']>=g['threshold'] and g['value']==42)
    if prize=='LP-0003':
        distributions=report['distributions']
        return len({d['address'] for d in distributions})>=2 and sum(d['state']['unique_claims'] for d in distributions)>=20
    raise ValueError('Unknown prize')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--require',choices=['LP-0002','LP-0003'])
    parser.add_argument('--output',type=Path)
    args=parser.parse_args()
    manifest=json.loads((ROOT/'release/manifest.json').read_text())
    deployment=json.loads((ROOT/'release/deployment.json').read_text())
    if deployment['endpoint']!=ENDPOINT or manifest['testnet_endpoint']!=ENDPOINT:
        raise ValueError('Unexpected release endpoint')
    if rpc('getProgramIds',[]).get('privacy_preserving_circuit')!=PROTOCOL:
        raise RuntimeError('Public protocol fingerprint differs from this release')
    first=rpc('getLastBlockId',[]);distributions=[]
    for address in deployment['distributions']:
        account_hex(address)
        account=rpc('getAccount',[address])
        if account['program_owner']!=manifest['programs']['allowlist']['image_id']:
            raise InvalidState('Distribution owner differs from published program')
        distributions.append({'address':address,'hex':account_hex(address),'state':decode_distribution(bytes(account['data']))})
    address=deployment['threshold_group'];account_hex(address);account=rpc('getAccount',[address])
    if account['program_owner']!=manifest['programs']['threshold']['image_id']:
        raise InvalidState('Group owner differs from published program')
    group={'address':address,'hex':account_hex(address),'state':decode_group(bytes(account['data']))}
    last=rpc('getLastBlockId',[])
    report={'checked_at':datetime.now(timezone.utc).isoformat(),'endpoint':ENDPOINT,
            'observed_block_range':[first,last],'wallet_loaded':False,'transactions_submitted':0,
            'programs':{k:v['image_id'] for k,v in manifest['programs'].items()},
            'distributions':distributions,'threshold':group}
    report['chain_criteria']={p:check_ready(report,p) for p in ['LP-0002','LP-0003']}
    report['not_checked_here']=['narrated video','green standalone proof CI','authorship and legal eligibility','prize award']
    text=json.dumps(report,indent=2)+'\n';print(text,end='')
    if args.output:
        output=args.output.resolve()
        if not output.is_relative_to(ROOT):raise ValueError('Evidence output must stay inside the repository')
        output.parent.mkdir(parents=True,exist_ok=True)
        temporary=output.with_suffix('.tmp');temporary.write_text(text);temporary.replace(output)
    if args.require and not report['chain_criteria'][args.require]:raise SystemExit(2)

if __name__=='__main__':main()
