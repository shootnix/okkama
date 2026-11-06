from pathlib import Path
import subprocess,os,json,time,statistics,random,re
repo=Path('/Volumes/SamsungSSD/stuff/okkama-new');out=Path('/tmp/okkama-opt-recheck-20261101');old=Path('/tmp/okkama-audit-20261101');vm=repo/'okkama-rs/target/release/sparrow'
def run(args,env=None,timeout=40):
 t=time.perf_counter();p=subprocess.run(list(map(str,args)),capture_output=True,env=env,timeout=timeout);dt=time.perf_counter()-t
 assert p.returncode==0,(args,p.returncode,p.stderr[-2000:]);return dt,p.stdout.decode(),p.stderr.decode()
def environ(mode):
 env=dict(os.environ)
 for k in ['OKKAMA_JIT','OKKAMA_JIT_THRESHOLD','OKKAMA_JIT_STATS']:env.pop(k,None)
 if mode!='default':env['OKKAMA_JIT']=mode
 return env
variants=[('original',old/'09-closures.spb')]+[(n,out/(n+'.spb')) for n in ['closures-extra-16','closures-extra-128','closures-extra-512','closures-extra-513','closures-extra-513-not-target']]
rows=[];expected=None
for repeat in range(4):
 for name,p in (variants if repeat%2==0 else list(reversed(variants))):
  sec,stdout,stderr=run([vm,p],environ('default'))
  if expected is None:expected=stdout
  assert stdout==expected,(name,stdout,expected)
  if repeat:rows.append(dict(variant=name,repeat=repeat,seconds=sec,stdout=stdout))
  print('closure',repeat,name,round(sec,4),flush=True)
(out/'closure-global-frame.json').write_text(json.dumps(rows,indent=2))
for name,p in variants:
 env=environ('default');env['OKKAMA_JIT_STATS']='1';_,stdout,stderr=run([vm,p],env);assert stdout==expected
 (out/(name+'-stats.log')).write_text(stderr)
# off controls, one execution each; not used for performance comparisons
for name,p in variants:
 _,stdout,_=run([vm,p],environ('off'));assert stdout==expected
print('closure controls and stats passed',flush=True)
rows=[];rng=random.Random(20261101)
for name in ['04-bigint','06-collections','09-closures','10-app']:
 expected=None
 for repeat in range(8):
  modes=['off','default'];rng.shuffle(modes)
  for mode in modes:
   sec,stdout,_=run([vm,old/(name+'.spb')],environ(mode))
   if expected is None:expected=stdout
   assert stdout==expected
   if repeat:rows.append(dict(bench=name,mode=mode,repeat=repeat,seconds=sec,stdout=stdout))
 print('baseline',name,[(m,round(statistics.median(r['seconds'] for r in rows if r['bench']==name and r['mode']==m),4)) for m in ['off','default']],flush=True)
(out/'benchmarks-repeat.json').write_text(json.dumps(rows,indent=2))
rows=[]
for name in ['04-bigint','06-collections','09-closures','10-app']:
 for mode in ['off','default']:
  for repeat in range(3):
   _,stdout,stderr=run([out/'alloc_probe',old/(name+'.spb')],environ(mode));values={k:int(v) for k,v in re.findall(r'(alloc_calls|requested_bytes|peak_extra_live_bytes|retained_delta)=(-?\d+)',stderr)}
   assert len(values)==4;rows.append(dict(bench=name,mode=mode,repeat=repeat,stdout=stdout,**values))
  print('alloc',name,mode,values,flush=True)
(out/'allocation-counters.json').write_text(json.dumps(rows,indent=2))
rows=[]
for mode in ['ascii','unicode']:
 for n in [4096,8192,16384,32768]:
  for repeat in range(3):
   _,stdout,_=run([out/'micro_probe',mode,n]);values={k:float(v) for k,v in re.findall(r'(repeated_slice|iterator)=([\d.]+)',stdout)}
   rows.append(dict(mode=mode,n=n,repeat=repeat,**values))
  print('strings',mode,n,values,flush=True)
(out/'string-scaling.json').write_text(json.dumps(rows,indent=2))
rows=[]
for n in [1048576,4194304]:
 for mode in ['rawbytes','bytes']:
  for repeat in range(3):
   p=subprocess.Popen([str(out/'micro_probe'),mode,str(n)],stdout=subprocess.PIPE,stderr=subprocess.PIPE);_,status,ru=os.wait4(p.pid,0);p.returncode=os.waitstatus_to_exitcode(status);p.communicate();assert p.returncode==0
   rows.append(dict(n=n,mode=mode,repeat=repeat,rss_bytes=ru.ru_maxrss))
print('byte memory checks done',flush=True);(out/'byte-memory.json').write_text(json.dumps(rows,indent=2))
subprocess.run(['python3',str(out/'imports_probe.py')],check=True)
print('ALL CHECKS COMPLETE',flush=True)
