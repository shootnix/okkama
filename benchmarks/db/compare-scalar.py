"""Same-binary scalar replacement A/B on precompiled SPB."""
import argparse, hashlib, json, os, statistics, subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--vm',required=True,type=Path)
p.add_argument('--origin',type=Path,help='Optional saved pre-change binary for the off samples')
p.add_argument('--objects',required=True,type=Path)
p.add_argument('--db',required=True,type=Path)
p.add_argument('--out',required=True,type=Path)
a=p.parse_args()
env=dict(os.environ,SCALAR_COUNT='200000',DB_BENCH_ROWS='5000',DB_BENCH_LOOKUPS='500',DB_BENCH_WARMUPS='4',OKKAMA_NO_REGIONS_BENCH='0',OKKAMA_SQLITE_INLINE_BENCH='0')
for k in ('OKKAMA_JIT','OKKAMA_JIT_STATS','OKKAMA_JIT_THRESHOLD','OKKAMA_GC','DB_BENCH_SQLITE_CACHE','OKKAMA_JIT_DUMP','OKKAMA_JIT_ONLY','OKKAMA_JIT_SKIP'):
    env.pop(k,None)
r={'method':'Standalone SPB, same normal release binary; nine alternating rounds for 200000 object computations after 3000 warmups; five alternating DB rounds after four warmups, worker enabled. No allocation instrumentation. Phase timers exclude loading and initial compilation, hot compilation can still affect DB phases.','sha256':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in (a.vm,a.objects,a.db)},'samples':[]}
if a.origin:
    r['origin_sha256']=hashlib.sha256(a.origin.read_bytes()).hexdigest()
    r['method']=r['method'].replace('same normal release binary','saved origin versus new normal release binary')
expected=json.loads((Path(__file__).parent/'results/pre-fused-fetch-python.json').read_text())['expected']
for work,path,rounds in [('objects',a.objects,9),('db',a.db,5)]:
    for n in range(rounds):
        for mode in (['off','on'] if n%2==0 else ['on','off']):
            run=subprocess.run([str(a.origin if mode=='off' and a.origin else a.vm),str(path)],env=dict(env,OKKAMA_JIT_SCALAR=mode),text=True,capture_output=True,check=True,timeout=180)
            if work=='objects':
                ns,checksum=map(int,run.stdout.split());assert checksum==200000*200005//2
                ms={'objects':ns/1e6}
            else:
                ms={}
                for line in run.stdout.splitlines():
                    phase,ns,total=line.split();assert int(total)==expected[phase];ms[phase]=int(ns)/1e6
                assert ms.keys()==expected.keys()
            r['samples'].append(dict(work=work,round=n,mode=mode,ms=ms))
            print(work,n,mode,ms,flush=True)
r['medians']={work:{mode:{phase:statistics.median(s['ms'][phase] for s in r['samples'] if s['work']==work and s['mode']==mode) for phase in (['objects'] if work=='objects' else expected)} for mode in ('off','on')} for work in ('objects','db')}
a.out.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(r['medians'],indent=2))
