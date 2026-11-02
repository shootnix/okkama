#!/usr/bin/env python3
"""Compare direct synchronous rusqlite, Python and Okkama's two delivery modes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import sqlite3
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--rust', type=Path, default=ROOT/'okkama-rs/target/release/examples/sqlite_sync')
    p.add_argument('--compiler', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    env = dict(os.environ, OKKAMA_STDLIB_DIR=str(ROOT/'std'), DB_BENCH_ROWS='5000',
               DB_BENCH_LOOKUPS='500', DB_BENCH_WARMUPS='4')
    for key in ('OKKAMA_JIT','OKKAMA_JIT_THRESHOLD','OKKAMA_ENGINE','OKKAMA_GC','DB_BENCH_SQLITE_CACHE','OKKAMA_SQLITE_INLINE_BENCH'):
        env.pop(key,None)
    for mode in ('worker','inline'):
        probe = subprocess.run([str(args.compiler),'run','--engine','sparrow',str(HERE/'inline-probe.ok')],
                               env=dict(env,OKKAMA_SQLITE_INLINE_BENCH='1' if mode=='inline' else '0'),
                               text=True,capture_output=True,timeout=60)
        if probe.returncode or probe.stdout.strip()!=mode:
            raise RuntimeError(probe.stdout+probe.stderr)
    expected = json.loads((HERE/'results/pre-fused-fetch-python.json').read_text())['expected']
    result = dict(method='Five sequential rotated rounds; four warmups; 5000 rows, 500 lookups/inserts. Rust materializes owned [rusqlite::Value;3] rows; it is a library-level floor, not an implementation of Okkama Row/error/lifecycle contracts. ASCII fixture makes byte length and character length equal.',
                  config=dict(runs=5,warmups=4,rows=5000,lookups=500),
                  environment=dict(platform=platform.platform(),python=sys.version,python_sqlite=sqlite3.sqlite_version,
                                   rust_binary_sha256=digest(args.rust),compiler_sha256=digest(args.compiler)),
                  sources={str(x.relative_to(ROOT)):digest(x) for x in [HERE/'main.py',HERE/'main.ok',Path(__file__),ROOT/'okkama-rs/crates/okkama-runtime-core/examples/sqlite_sync.rs']},
                  expected=expected,samples=[])
    modes=['rusqlite','python','worker','inline']
    args.out.parent.mkdir(parents=True,exist_ok=True)
    for i in range(5):
        for mode in modes[i%4:]+modes[:i%4]:
            run_env=dict(env,OKKAMA_SQLITE_INLINE_BENCH='1' if mode=='inline' else '0')
            command=([str(args.rust)] if mode=='rusqlite' else [sys.executable,str(HERE/'main.py')] if mode=='python'
                     else [str(args.compiler),'run','--engine','sparrow',str(HERE/'main.ok')])
            r=subprocess.run(command,cwd=ROOT,env=run_env,text=True,capture_output=True,timeout=180)
            if r.returncode:raise RuntimeError(r.stdout+r.stderr)
            phases={}
            for line in r.stdout.splitlines():
                phase,ns,total=line.split();assert int(total)==expected[phase],(mode,phase,total)
                phases[phase]=int(ns)/1e6
            assert phases.keys()==expected.keys()
            result['samples'].append(dict(iteration=i,mode=mode,ms=phases,stderr=r.stderr))
            args.out.write_text(json.dumps(result,indent=2)+'\n')
            print(i,mode,phases,flush=True)
    result['summary']={mode:{phase:statistics.median(s['ms'][phase] for s in result['samples'] if s['mode']==mode)
                             for phase in expected} for mode in modes}
    args.out.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result['summary'],indent=2))

if __name__=='__main__':main()
