"""Diagnostic region semantics ablation. Not a production-compatible mode."""
import argparse,hashlib,json,os,statistics,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parent.parent
p=argparse.ArgumentParser()
p.add_argument('--compiler',required=True,type=Path)
p.add_argument('--out',required=True,type=Path)
a=p.parse_args()
env=dict(os.environ,OKKAMA_STDLIB_DIR=str(ROOT/'std'),DB_BENCH_ROWS='5000',DB_BENCH_LOOKUPS='500',DB_BENCH_WARMUPS='4')
for k in ('OKKAMA_JIT','OKKAMA_JIT_STATS','OKKAMA_JIT_THRESHOLD','OKKAMA_GC','DB_BENCH_SQLITE_CACHE'):env.pop(k,None)
probes=[]
for engine in ('ir','sparrow'):
 for disabled in (0,1):
  e=dict(env,OKKAMA_NO_REGIONS_BENCH=str(disabled))
  run=subprocess.run([str(a.compiler),'run','--engine',engine,str(HERE/'no-regions-probe.ok')],env=e,text=True,capture_output=True,check=True)
  expected='no-regions' if disabled else 'regions'
  assert run.stdout.strip()==expected,(engine,disabled,run.stdout,run.stderr)
  probes.append(dict(engine=engine,disabled=disabled,stdout=run.stdout))
for inline in (0,1):
 run=subprocess.run([str(a.compiler),'run','--engine','sparrow',str(HERE/'inline-probe.ok')],env=dict(env,OKKAMA_SQLITE_INLINE_BENCH=str(inline)),text=True,capture_output=True,check=True)
 assert run.stdout.strip()==('inline' if inline else 'worker')
expected=json.loads((HERE/'results/pre-fused-fetch-python.json').read_text())['expected']
modes=['regions_worker','disabled_worker','regions_inline','disabled_inline','python']
r={'method':'Same binary; independent processes, five rotated rounds, four warmups, 5000 rows/500 lookups. Disabled mode deliberately omits region ownership and automatic destroy; explicit cleanup, Rc and cycle GC remain. Region objects and bytecode still exist.', 'compiler_sha256':hashlib.sha256(a.compiler.read_bytes()).hexdigest(),'probes':probes,'samples':[]}
for n in range(5):
 for mode in modes[n:]+modes[:n]:
  e=dict(env,OKKAMA_SQLITE_INLINE_BENCH='1' if mode.endswith('inline') else '0',OKKAMA_NO_REGIONS_BENCH='1' if mode.startswith('disabled') else '0')
  cmd=[sys.executable,str(HERE/'main.py')] if mode=='python' else [str(a.compiler),'run','--engine','sparrow',str(HERE/'main.ok')]
  run=subprocess.run(cmd,env=e,text=True,capture_output=True,check=True,timeout=180)
  ms={}
  for line in run.stdout.splitlines():
   phase,ns,total=line.split();assert int(total)==expected[phase];ms[phase]=int(ns)/1e6
  assert ms.keys()==expected.keys()
  r['samples'].append(dict(round=n,mode=mode,ms=ms))
  print(n,mode,ms,flush=True)
r['medians']={m:{p:statistics.median(s['ms'][p] for s in r['samples'] if s['mode']==m) for p in expected} for m in modes}
r['sources_sha256']={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [Path(__file__),HERE/'main.ok',HERE/'no-regions-probe.ok',ROOT/'okkama-rs/crates/okkama-runtime-core/src/heap.rs']}
a.out.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(r['medians'],indent=2))
