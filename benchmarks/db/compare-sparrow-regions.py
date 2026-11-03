"""Region ablation on precompiled SPB, without compiler/IR in the process."""
import argparse, hashlib, json, os, statistics, subprocess, sys
from pathlib import Path
HERE = Path(__file__).resolve().parent
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--vm', required=True, type=Path)
p.add_argument('--bytecode', required=True, type=Path)
p.add_argument('--region-probe', required=True, type=Path)
p.add_argument('--inline-probe', required=True, type=Path)
p.add_argument('--out', required=True, type=Path)
a = p.parse_args()
env = dict(os.environ, DB_BENCH_ROWS='5000', DB_BENCH_LOOKUPS='500', DB_BENCH_WARMUPS='4')
for k in ('OKKAMA_JIT','OKKAMA_JIT_STATS','OKKAMA_JIT_THRESHOLD','OKKAMA_GC','DB_BENCH_SQLITE_CACHE'):
    env.pop(k, None)
for flag in (0,1):
    e = dict(env, OKKAMA_NO_REGIONS_BENCH=str(flag), OKKAMA_SQLITE_INLINE_BENCH=str(flag))
    for path, expected in ((a.region_probe, 'no-regions' if flag else 'regions'), (a.inline_probe, 'inline' if flag else 'worker')):
        run = subprocess.run([str(a.vm),str(path)], env=e, text=True, capture_output=True, check=True)
        assert run.stdout.strip() == expected, (run.stdout,run.stderr)
expected = json.loads((HERE/'results/pre-fused-fetch-python.json').read_text())['expected']
modes = ['regions_worker','disabled_worker','regions_inline','disabled_inline','python']
r = {'method':'Standalone Sparrow executes identical precompiled SPB. Five rotated rounds, four warmups; 5000 rows/500 queries. Same VM binary for all Sparrow modes. Disabling regions changes destroy semantics; inline SQLite blocks the event loop. Diagnostic modes only.', 'sha256':{str(x):hashlib.sha256(x.read_bytes()).hexdigest() for x in (a.vm,a.bytecode,a.region_probe,a.inline_probe)},'samples':[]}
for n in range(5):
    for mode in modes[n:]+modes[:n]:
        e = dict(env, OKKAMA_NO_REGIONS_BENCH='1' if mode.startswith('disabled') else '0', OKKAMA_SQLITE_INLINE_BENCH='1' if mode.endswith('inline') else '0')
        cmd = [sys.executable,str(HERE/'main.py')] if mode=='python' else [str(a.vm),str(a.bytecode)]
        run = subprocess.run(cmd,env=e,text=True,capture_output=True,check=True,timeout=180)
        ms = {}
        for line in run.stdout.splitlines():
            phase,ns,total = line.split()
            assert int(total)==expected[phase]
            ms[phase]=int(ns)/1e6
        assert ms.keys()==expected.keys()
        r['samples'].append(dict(round=n,mode=mode,ms=ms))
        print(n,mode,ms,flush=True)
r['medians']={m:{phase:statistics.median(s['ms'][phase] for s in r['samples'] if s['mode']==m) for phase in expected} for m in modes}
a.out.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(r['medians'],indent=2))
