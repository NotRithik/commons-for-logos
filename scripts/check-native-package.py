#!/usr/bin/env python3
"""Verify LGX integrity and install into a new isolated test directory only."""
import argparse
import ctypes as c
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('packager',ROOT/'scripts/package-lgx.py')
packager=importlib.util.module_from_spec(spec);spec.loader.exec_module(packager)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package',type=Path,required=True)
    parser.add_argument('--frameworks',type=Path,required=True)
    parser.add_argument('--out',type=Path,default=ROOT/'out/package-check')
    args=parser.parse_args()
    out=args.out.resolve()
    if out.exists() or out==ROOT or not out.is_relative_to(ROOT):
        parser.error('choose a new output directory inside this checkout')
    out.mkdir(parents=True,mode=0o700)
    lgx=packager.Lgx(args.frameworks.resolve()/'liblgx.dylib')
    original=lgx.verify(args.package.resolve())
    if not original['valid']:raise RuntimeError(original)
    # Corrupt only a private copy of our own package. Original is retained.
    entries=[]
    with tarfile.open(args.package,'r:gz') as tar:
        for member in tar.getmembers():
            if member.isfile():
                content=tar.extractfile(member).read()
                if member.name.endswith('/qml/Main.qml'):content+=b'\n// integrity test\n'
                entries.append((member.name,content))
    broken=out/'tampered-test-copy.lgx'
    with tarfile.open(broken,'w:gz') as tar:
        for name,content in entries:
            info=tarfile.TarInfo(name);info.size=len(content);info.mode=0o644
            tar.addfile(info,io.BytesIO(content))
    tampered=lgx.verify(broken)
    if tampered['valid']:raise RuntimeError('Modified content was incorrectly accepted')
    lib=c.CDLL(str(args.frameworks.resolve()/'libpackage_manager_lib.dylib'))
    signatures=[('lgpm_create',[],c.c_void_p),('lgpm_free',[c.c_void_p],None),
       ('lgpm_set_user_modules_dir',[c.c_void_p,c.c_char_p],None),
       ('lgpm_set_user_ui_plugins_dir',[c.c_void_p,c.c_char_p],None),
       ('lgpm_install_file',[c.c_void_p,c.c_char_p,c.c_bool,c.POINTER(c.c_void_p),c.POINTER(c.c_bool)],c.c_void_p),
       ('lgpm_get_last_error',[],c.c_char_p),('lgpm_free_string',[c.c_void_p],None)]
    for name,params,result in signatures:
        fn=getattr(lib,name);fn.argtypes=params;fn.restype=result
    ctx=lib.lgpm_create()
    if not ctx:raise RuntimeError('Cannot create package manager context')
    directory=plugin=None
    try:
        lib.lgpm_set_user_modules_dir(ctx,str(out/'modules').encode())
        lib.lgpm_set_user_ui_plugins_dir(ctx,str(out/'plugins').encode())
        plugin=c.c_void_p();is_core=c.c_bool()
        directory=lib.lgpm_install_file(ctx,str(args.package.resolve()).encode(),False,c.byref(plugin),c.byref(is_core))
        if not directory:raise RuntimeError((lib.lgpm_get_last_error() or b'Package install failed').decode())
        returned=Path(c.string_at(directory).decode()).resolve()
        if not returned.is_relative_to(out/'plugins'):raise RuntimeError('Installed outside designated UI directory')
        main=Path(c.string_at(plugin).decode()).resolve()
        installed=main.parent
        if installed != out/'plugins'/packager.NAME or not main.is_file() or is_core.value:
            raise RuntimeError('Incorrect installed plugin identity')
        with tarfile.open(args.package,'r:gz') as tar:
            count=0
            for member in tar.getmembers():
                if not member.isfile() or not member.name.startswith('variants/darwin-arm64/'):continue
                relative=member.name.removeprefix('variants/darwin-arm64/')
                target=installed/relative
                if target.read_bytes()!=tar.extractfile(member).read():raise RuntimeError('Installed file differs: '+relative)
                count+=1
        report={'package':args.package.name,'original_verification':original,
                'tampering_rejected':not tampered['valid'],'tamper_errors':tampered['errors'],
                'install_type':'ui_qml','installed_file_count':count,
                'main_plugin':main.name,'installation':'new isolated directory','plugin_executed_by_this_test':False}
        (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps(report,indent=2))
    finally:
        if directory:lib.lgpm_free_string(directory)
        if plugin and plugin.value:lib.lgpm_free_string(plugin)
        lib.lgpm_free(ctx)

if __name__=='__main__':main()
