#!/usr/bin/env python3
"""Package only this project's native module, using the official LGX library.

Run with a trusted liblgx from the Logos Basecamp installation or a reviewed
source build. No dependency installation or signing-policy change is performed.
"""
from __future__ import annotations
import argparse
import ctypes as c
import hashlib
import io
import json
from pathlib import Path
import shutil
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
NAME = 'commons_primitives_ui'

class Result(c.Structure):
    _fields_ = [('success', c.c_bool), ('error', c.c_char_p)]
class Verification(c.Structure):
    _fields_ = [('valid',c.c_bool), ('errors',c.POINTER(c.c_char_p)), ('warnings',c.POINTER(c.c_char_p))]

def strings(values):
    out=[]
    if values:
        for i in range(100):
            if not values[i]:break
            out.append(values[i].decode('utf-8','replace'))
    return out

class Lgx:
    def __init__(self,path):
        self.lib=c.CDLL(str(path))
        for name,args,result in [
            ('lgx_load',[c.c_char_p],c.c_void_p),
            ('lgx_add_variant',[c.c_void_p,c.c_char_p,c.c_char_p,c.c_char_p],Result),
            ('lgx_save',[c.c_void_p,c.c_char_p],Result),
            ('lgx_verify',[c.c_char_p],Verification),
            ('lgx_get_last_error',[],c.c_char_p),
            ('lgx_free_package',[c.c_void_p],None),
            ('lgx_free_verify_result',[Verification],None),
        ]:
            f=getattr(self.lib,name);f.argtypes=args;f.restype=result
    def check(self,result):
        if not result.success:raise RuntimeError((result.error or b'LGX operation failed').decode())
    def verify(self,path):
        result=self.lib.lgx_verify(str(path).encode())
        try:return {'valid':bool(result.valid),'errors':strings(result.errors),'warnings':strings(result.warnings)}
        finally:self.lib.lgx_free_verify_result(result)

def files_for_variant(native:Path,variant:str):
    if variant not in ['darwin-arm64','linux-x86_64']:raise ValueError('Unsupported native variant')
    ext='.dylib' if variant=='darwin-arm64' else '.so'
    names=[NAME+'_plugin'+ext,NAME+'_replica_factory'+ext,'metadata.json','qml/Main.qml','qml/WorkspacePicker.qml','qml/WorkspaceState.js','qml/GovernancePage.qml','icons/commons.svg']
    for name in names:
        p=native/name
        if p.is_symlink() or not p.is_file():raise ValueError('Missing regular module file: '+name)
        if not p.resolve().is_relative_to(native.resolve()):raise ValueError('File escaped module directory')
    metadata=json.loads((native/'metadata.json').read_text())
    if metadata.get('name')!=NAME or metadata.get('type')!='ui_qml' or metadata.get('view')!='qml/Main.qml':
        raise ValueError('Unexpected module metadata')
    return names,metadata

def seed_archive(path:Path,manifest:dict,root_files:dict[str,bytes]):
    with tarfile.open(path,'w:gz',format=tarfile.USTAR_FORMAT) as tar:
        for name,data in {'manifest.json':json.dumps(manifest).encode(),**root_files}.items():
            info=tarfile.TarInfo(name);info.size=len(data);info.mode=0o644;info.mtime=0
            tar.addfile(info,io.BytesIO(data))

def package(lgx:Lgx,native:Path,variant:str,output:Path):
    names,metadata=files_for_variant(native,variant)
    if output.exists():raise ValueError('Refusing to replace an existing package')
    output.parent.mkdir(parents=True,exist_ok=True)
    ext='.dylib' if variant=='darwin-arm64' else '.so'
    manifest={'manifestVersion':'0.3.0','name':NAME,'display_name':'Commons',
              'version':metadata['version'],'description':'Private membership and shared approvals on Logos testnet.',
              'author':'NotRithik','type':'ui_qml','category':'governance','dependencies':[],
              'main':{variant:NAME+'_plugin'+ext},'view':'qml/Main.qml','icon':'icons/commons.svg'}
    root_files={}
    with tempfile.TemporaryDirectory(prefix='lgx-build-',dir=output.parent) as temp:
        temp=Path(temp);seed=temp/'seed.lgx';payload=temp/'payload';payload.mkdir()
        for name in names:
            p=payload/name;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(native/name,p)
        for license_name in ['LICENSE-MIT','LICENSE-APACHE']:
            shutil.copyfile(ROOT/license_name,payload/license_name)
        seed_archive(seed,manifest,root_files)
        handle=lgx.lib.lgx_load(str(seed).encode())
        if not handle:raise RuntimeError(lgx.lib.lgx_get_last_error().decode())
        try:
            lgx.check(lgx.lib.lgx_add_variant(handle,variant.encode(),str(payload).encode(),(NAME+'_plugin'+ext).encode()))
            candidate=temp/'verified.lgx'
            lgx.check(lgx.lib.lgx_save(handle,str(candidate).encode()))
            result=lgx.verify(candidate)
            if not result['valid']:raise RuntimeError('LGX verification failed: '+str(result['errors']))
            candidate.replace(output)
        finally:lgx.lib.lgx_free_package(handle)
    data=output.read_bytes()
    return {'file':output.name,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),
            'variant':variant,'files':names+['LICENSE-MIT','LICENSE-APACHE'],'verification':result}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--lgx-lib',type=Path,required=True)
    p.add_argument('--native-dir',type=Path,required=True)
    p.add_argument('--variant',choices=['darwin-arm64','linux-x86_64'],required=True)
    p.add_argument('--output',type=Path,required=True)
    args=p.parse_args()
    report=package(Lgx(args.lgx_lib.resolve()),args.native_dir.resolve(),args.variant,args.output.resolve())
    print(json.dumps(report,indent=2))

if __name__=='__main__':main()
