"""Paired scratch-buffer benchmark; no allocation instrumentation during timings."""
import argparse, hashlib, json, os, statistics, subprocess, sys
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parent.parent
p=argparse.ArgumentParser()
p.add_argument('--before',required=True)
p.add_argument('--after',required=True)
p.add_argument('--out',required=True)
a=p.parse_args()
env=dict(os.environ,OKKAMA_STDLIB_DIR=str(ROOT/'std'),DB_BENCH_ROWS='5000',DB_BENCH_LOOKUPS='500',DB_BENCH_WARMUPS='4')
for k in ('OKKAMA_JIT','OKKAMA_JIT_STATS','OKKAMA_JIT_THRESHOLD','OKKAMA_GC','DB_BENCH_SQLITE_CACHE'):env.pop(k,None)
expected=json.loads((HERE/'results/pre-fused-fetch-python.json').read_text())['expected']
modes=['before_worker','after_worker','before_inline','after_inline','python']
r={'method':'Five sequential rotated rounds; four phase warmups; 5000 rows, 500 lookups/inserts; same stdlib; no diagnostic allocation counters.', 'binaries':{k:hashlib.sha256(Path(v).read_bytes()).hexdigest() for k,v in [('before',a.before),('after',a.after)]},'samples':[]}
for n in range(5):
 for mode in modes[n:]+modes[:n]:
  e=dict(env,OKKAMA_SQLITE_INLINE_BENCH='1' if mode.endswith('inline') else '0')
  cmd=[sys.executable,str(HERE/'main.py')] if mode=='python' else [a.before if mode.startswith('before') else a.after,'run','--engine','sparrow',str(HERE/'main.ok')]
  run=subprocess.run(cmd,env=e,text=True,capture_output=True,check=True,timeout=180)
  ms={}
  for line in run.stdout.splitlines():
   phase,ns,total=line.split();assert int(total)==expected[phase];ms[phase]=int(ns)/1e6
  assert ms.keys()==expected.keys()
  r['samples'].append(dict(round=n,mode=mode,ms=ms))
  print(n,mode,ms['lookup_prepared'],flush=True)
r['medians']={m:{p:statistics.median(s['ms'][p] for s in r['samples'] if s['mode']==m) for p in expected} for m in modes}
Path(a.out).write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(r['medians'],indent=2))
