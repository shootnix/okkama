#!/usr/bin/env python3
"""Sequential SQLite API benchmark; phase timings exclude startup/build.

Each program warms each phase once before measuring it. Engine order rotates
between samples; every checksum is independently derived and checked.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--rows', type=int, default=5000)
    parser.add_argument('--lookups', type=int, default=500)
    parser.add_argument('--engines', default='python,ir,sparrow,jvm')
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--compiler', type=Path, default=ROOT/'okkama-rs/target/release/okkama')
    args = parser.parse_args()
    if min(args.runs,args.rows,args.lookups) < 1:
        parser.error('runs, rows and lookups must be positive')
    engines = args.engines.split(',')
    if set(engines)-{'python','ir','sparrow','jvm'}:
        parser.error('unknown engine')
    env = {k:v for k,v in os.environ.items() if k not in ('OKKAMA_JIT','OKKAMA_ENGINE','OKKAMA_GC')}
    env.update(OKKAMA_STDLIB_DIR=str(ROOT/'std'),DB_BENCH_ROWS=str(args.rows),DB_BENCH_LOOKUPS=str(args.lookups))
    def row(i):return i+len('user-'+str(i))+(5 if i%3==0 else 0 if i%5==0 else 4)
    expected = dict(projection_only=args.rows*row(0),scan_raw=args.rows*3,scan_stream=sum(map(row,range(args.rows))),scan_all=sum(map(row,range(args.rows))),lookup_prepared=sum(row(i%args.rows) for i in range(args.lookups)),lookup_statement=sum(row(i%args.rows) for i in range(args.lookups)),insert_transaction=args.lookups*(args.lookups-1)//2)
    results = {'config':{'runs':args.runs,'rows':args.rows,'lookups':args.lookups,'engines':engines},'environment':{'platform':platform.platform(),'python':sys.version,'compiler_version':subprocess.check_output([str(args.compiler),'--version'],text=True).strip(),'compiler_sha256':digest(args.compiler),'git_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'stdlib_hashes':{str(p.relative_to(ROOT)):digest(p) for p in sorted((ROOT/'std/db').rglob('*.ok'))},'sources':{p.name:digest(p) for p in [HERE/'main.ok',HERE/'main.py',HERE/'run.py']}},'expected':expected,'samples':[]}
    for iteration in range(args.runs):
        order = engines[iteration%len(engines):]+engines[:iteration%len(engines)]
        for engine in order:
            cmd=[sys.executable,str(HERE/'main.py')] if engine=='python' else [str(args.compiler),'run','--engine',engine,str(HERE/'main.ok')]
            start=time.perf_counter()
            p=subprocess.run(cmd,cwd=ROOT,env=env,text=True,capture_output=True,timeout=180)
            if p.returncode:raise RuntimeError(f'{engine}: {p.stdout}\n{p.stderr}')
            phases={}
            for line in p.stdout.splitlines():
                name,ns,checksum=line.split()
                if int(checksum)!=expected[name]:raise AssertionError((engine,name,checksum,expected[name]))
                phases[name]={'ms':int(ns)/1e6,'checksum':int(checksum)}
            if phases.keys()!=expected.keys():raise AssertionError('missing phases')
            results['samples'].append({'engine':engine,'iteration':iteration,'process_seconds':time.perf_counter()-start,'phases':phases})
            args.out.parent.mkdir(parents=True,exist_ok=True)
            args.out.write_text(json.dumps(results,indent=2)+'\n')
            print(iteration+1,engine,', '.join(f'{k}={v["ms"]:.2f}ms' for k,v in phases.items()),flush=True)
    results['summary']={engine:{phase:{'median_ms':statistics.median(v:=[s['phases'][phase]['ms'] for s in results['samples'] if s['engine']==engine]),'min_ms':min(v),'max_ms':max(v)} for phase in expected} for engine in engines}
    args.out.write_text(json.dumps(results,indent=2)+'\n')

if __name__=='__main__':main()
