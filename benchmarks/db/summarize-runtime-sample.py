"""Main-thread stack observations; inclusive categories overlap."""
import argparse, collections, gzip, json, re
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('sample',type=Path);p.add_argument('--out',required=True,type=Path);a=p.parse_args()
s=gzip.decompress(a.sample.read_bytes()).decode() if a.sample.suffix=='.gz' else a.sample.read_text()
g=s.split('Call graph:\n')[1].split('Total number in stack')[0]
nodes=[];stack=[]
for line in g.splitlines():
    m=re.match(r'([ +|!:]*)(\d+) (.*)',line)
    if not m:continue
    depth,count,symbol=len(m[1]),int(m[2]),m[3]
    while stack and stack[-1]['depth']>=depth:stack.pop()
    if stack:stack[-1]['self']-=count
    n=dict(depth=depth,count=count,self=count,symbol=symbol,ancestors=[x['symbol'] for x in stack]);nodes.append(n);stack.append(n)
assert all(n['self']>=0 for n in nodes)
main=[n for n in nodes if any('com.apple.main-thread' in x for x in n['ancestors']+[n['symbol']])];total=sum(n['self'] for n in main)
assert total>0
patterns={'regions':'6Region','graph traversal':'4heap7collect','allocator':r'\b_?mi_|\bmalloc|\bfree\b','Value clone':'5Value.*5clone5Clone5clone','Value drop':'drop_glue.*5value5Value','scheduler own':'5sched','JIT call helpers':'12call_generic|6h_call|9h_call_dyn','memcpy':'memcpy|memmove','SQLite':'sqlite3','wait':'__psynch|__ulock|semaphore_wait|mach_msg'}
r={'main_thread_observations':total,'note':'Sampling observations, not exact CPU time; inclusive categories overlap. Scheduler inclusive covers task bodies and is not scheduler overhead. SQLite executes on another thread. Unknown JIT addresses and inlining limit attribution.','categories':{}}
for label,pattern in patterns.items():
    own=sum(n['self'] for n in main if re.search(pattern,n['symbol']));incl=sum(n['self'] for n in main if any(re.search(pattern,x) for x in n['ancestors']+[n['symbol']]))
    r['categories'][label]={'self':own,'inclusive':incl,'self_pct':round(100*own/total,2),'inclusive_pct':round(100*incl/total,2)}
top=collections.Counter()
for n in main:top[n['symbol'].split('  (in ')[0]]+=n['self']
r['top_self']=top.most_common(20)
a.out.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(r['categories'],indent=2))
