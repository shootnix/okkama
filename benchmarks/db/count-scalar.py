"""Allocation slopes, excluding fixed loading/JIT work; diagnostic binary only."""
import argparse, hashlib, json, os, re, subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--vm',required=True,type=Path)
p.add_argument('--bytecode',required=True,type=Path)
p.add_argument('--out',required=True,type=Path)
a=p.parse_args()
env=dict(os.environ)
for k in ('OKKAMA_JIT','OKKAMA_JIT_STATS','OKKAMA_JIT_THRESHOLD','OKKAMA_JIT_DUMP','OKKAMA_JIT_ONLY','OKKAMA_JIT_SKIP','OKKAMA_GC'):
    env.pop(k,None)
r={'method':'Standalone diagnostic debug binary, same allocator with request counters. Each sample warms 3000 calls. Slopes across 5000/10000/15000 calls remove fixed compilation/startup costs. Counts, not timings; excludes native C allocations and RSS.','sha256':{str(x):hashlib.sha256(x.read_bytes()).hexdigest() for x in (a.vm,a.bytecode)},'samples':[],'deltas':[]}
for repeat in range(2):
    for mode in ('off','on'):
        last=None
        for count in (5000,10000,15000):
            run=subprocess.run([str(a.vm),str(a.bytecode)],env=dict(env,OKKAMA_JIT_SCALAR=mode,SCALAR_COUNT=str(count)),text=True,capture_output=True,check=True)
            ns,total=map(int,run.stdout.split());assert total==count*(count+5)//2
            counters=json.loads(re.findall(r'allocation_stats (\[[^\n]+\])',run.stderr)[0])
            sample=dict(repeat=repeat,mode=mode,count=count,counters=counters)
            r['samples'].append(sample)
            if last:
                delta=[(x-y)/(count-last['count']) for x,y in zip(counters,last['counters'])]
                r['deltas'].append(dict(repeat=repeat,mode=mode,start=last['count'],end=count,per_call=delta))
                print(mode,repeat,delta,flush=True)
            last=sample
r['fields']=['alloc_calls','alloc_requested_bytes','realloc_calls','realloc_requested_bytes','dealloc_calls']
a.out.write_text(json.dumps(r,indent=2)+'\n')
