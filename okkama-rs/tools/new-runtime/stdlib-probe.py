#!/usr/bin/env python3
"""Compile and run every stdlib file containing inline tests on the new engine.
Failures and unsupported providers are recorded, never counted as passed tests.
"""
import argparse, hashlib, json, os, re, shutil, subprocess, tempfile
from pathlib import Path
from probe_process import run_bounded, ResourceLimitExceeded, positive_int, map_bounded
ROOT=Path(__file__).resolve().parents[3]
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--compiler',type=Path,required=True);p.add_argument('--runtime',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--jobs',type=positive_int,default=1)
a=p.parse_args();compiler=a.compiler.resolve()
# A probe must execute one immutable binary even while development builds run.
snapshot=tempfile.TemporaryDirectory(prefix='sparrow-stdlib-runtime-')
runtime=Path(snapshot.name)/'sparrow-next'
shutil.copy2(a.runtime.resolve(),runtime)
paths=[path for path in sorted((ROOT/'std').rglob('*.ok')) if re.search(r'^\s*(?:async\s+)?test\s+"',path.read_text(),re.M)]
def run(path):
    record={'path':str(path.relative_to(ROOT)),'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'runs':[]}
    with tempfile.TemporaryDirectory(prefix='sparrow-stdlib-') as work:
        binary=Path(work)/'tests.spb';env=dict(os.environ,OKKAMA_TRACE='off',OKKAMA_STDLIB_DIR=str(ROOT/'std'),TMPDIR=work)
        try:
            built=run_bounded([str(compiler),'build','-t',str(path),'--target','sparrow','-o',str(binary)],cwd=work,env=env,stdin=subprocess.DEVNULL,capture_output=True,timeout=60)
            if built.returncode:
                record.update(status='COMPILE_FAILURE',stderr=built.stderr.decode('utf-8','replace'));return record
            record['spb_sha256']=hashlib.sha256(binary.read_bytes()).hexdigest()
            for mode,flags in [('cold',['--cold']),('jit',[])]:
                try:
                    result=run_bounded([str(runtime),'--test',*flags,str(binary)],cwd=work,env=env,stdin=subprocess.DEVNULL,capture_output=True,timeout=60)
                    stdout=result.stdout.decode('utf-8','replace');stderr=result.stderr.decode('utf-8','replace')
                    record['runs'].append({'mode':mode,'exit':result.returncode,'status':'PASS' if result.returncode==0 else 'FAIL','passed_cases':len(re.findall(r'^--- PASS:',stdout,re.M)),'failed_cases':len(re.findall(r'^--- FAIL:',stdout,re.M)),'stdout':stdout,'stderr':stderr})
                except (subprocess.TimeoutExpired, ResourceLimitExceeded) as error:
                    record['runs'].append({'mode':mode,'status':getattr(error,'reason','TIMEOUT'),'stdout':(error.stdout or b'').decode('utf-8','replace'),'stderr':(error.stderr or b'').decode('utf-8','replace')})
        except (subprocess.TimeoutExpired, ResourceLimitExceeded) as error:
            record.update(status=getattr(error,'reason','COMPILE_TIMEOUT'))
    return record
records=[]
for record in map_bounded(run, paths, a.jobs):
    records.append(record);print(f"[{len(records)}/{len(paths)}] {record.get('status') or '/'.join(r['status'] for r in record['runs'])} {record['path']}",flush=True)
summary={mode:sum(any(r['mode']==mode and r['status']=='PASS' for r in record['runs']) for record in records) for mode in ['cold','jit']}
a.output.parent.mkdir(parents=True,exist_ok=True)
a.output.write_text(json.dumps({'scope':'all stdlib files containing inline tests; compile and new-runtime execution','compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest(),'runtime_sha256':hashlib.sha256(runtime.read_bytes()).hexdigest(),'files':len(paths),'passed_files':summary,'results':records},ensure_ascii=False,indent=2)+'\n')
print(summary)
raise SystemExit(0 if all(count==len(paths) for count in summary.values()) else 1)
