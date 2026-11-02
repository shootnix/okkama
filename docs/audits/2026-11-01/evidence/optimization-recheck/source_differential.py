from pathlib import Path
import subprocess,random,os,json
repo=Path('/Volumes/SamsungSSD/stuff/okkama-new');out=Path('/tmp/okkama-opt-recheck-20261101');rng=random.Random(202611012);rows=[];failures=[]
cli=repo/'okkama-rs/target/release/okkama';vm=repo/'okkama-rs/target/release/sparrow'
for case in range(60):
 n=rng.randrange(30,180);a=rng.randrange(2,101);b=rng.randrange(1,10000);mod=rng.choice([1000003,2**63+13,2**80+13]);seed=rng.randrange(1,mod)
 src=f'''module audit v1.0.0;
main() is int {{
 let f = (x is int) is int {{ (x * {a} + {b}) % {mod}; }};
 let result = (0..{n}).reduce({seed}) -> acc, i {{
  let following = if i % 3 == 0 {{ f(acc); }} else {{ (acc + i * {b}) % {mod}; }};
  let xs = [following, i, {b}];
  xs.reduce(0) -> sum, value {{ (sum + value) % {mod}; }};
 }};
 result % 251;
}}
'''
 acc=seed
 for i in range(n):
  acc=(acc*a+b)%mod if i%3==0 else (acc+i*b)%mod
  acc=(acc+i+b)%mod
 expected=acc%251;p=out/f'differential-{case}.ok';p.write_text(src);spb=p.with_suffix('.spb')
 r=subprocess.run([str(cli),'build',str(p),'--target','sparrow','-o',str(spb)],capture_output=True,text=True,timeout=15)
 if r.returncode:raise RuntimeError(r.stderr)
 results={}
 modes=[('ir',{}),('off',{'OKKAMA_JIT':'off'}),('eager',{'OKKAMA_JIT':'eager'}),('threshold2',{'OKKAMA_JIT_THRESHOLD':'2'}),('threshold10',{'OKKAMA_JIT_THRESHOLD':'10'})]
 for name,settings in modes:
  env=dict(os.environ)
  for k in ['OKKAMA_JIT','OKKAMA_JIT_THRESHOLD','OKKAMA_JIT_STATS']:env.pop(k,None)
  env.update(settings)
  args=[str(cli),'run','--engine','ir',str(p)] if name=='ir' else [str(vm),str(spb)]
  r=subprocess.run(args,env=env,capture_output=True,text=True,timeout=15);results[name]={'exit':r.returncode,'stderr':r.stderr}
 if any(r['exit']!=expected for r in results.values()):failures.append({'case':case,'expected':expected,'results':results})
 rows.append({'case':case,'expected':expected,'results':results})
 if case%10==9:print('source differential',case+1,'failures',len(failures),flush=True)
(out/'source-differential.json').write_text(json.dumps({'seed':202611012,'cases':60,'modes':5,'failures':failures,'results':rows},indent=2))
