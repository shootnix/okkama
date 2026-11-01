from pathlib import Path
import subprocess,random,os,json,time,resource
ROOT=Path('/Volumes/SamsungSSD/stuff/okkama-new'); OUT=Path('/tmp/okkama-audit-20261101'); VM=str(ROOT/'okkama-rs/target/release/sparrow'); rng=random.Random(11012026)
def call(args,env=None,timeout=3):
 try:
  p=subprocess.run([VM,*map(str,args)],capture_output=True,env=env,timeout=timeout,preexec_fn=lambda:resource.setrlimit(resource.RLIMIT_CORE,(0,0)))
  return dict(exit=p.returncode,stdout=p.stdout.decode(errors='replace'),stderr=p.stderr.decode(errors='replace'))
 except subprocess.TimeoutExpired:return dict(timeout=timeout)
seed=(OUT/'hello.spb').read_bytes(); count=1500; anomalies=[]; accepted=0
for i in range(count):
 b=bytearray(seed)
 mode=i%5
 if mode==0:b=bytearray(rng.randbytes(rng.randint(0,256)))
 elif mode==1:b=b[:rng.randrange(len(b)+1)]
 elif mode==2:
  for _ in range(rng.randint(1,4)):b[rng.randrange(len(b))]=rng.randrange(256)
 elif mode==3:
  j=rng.randrange(len(b)+1);b[j:j]=rng.randbytes(rng.randint(1,32))
 else:b+=rng.randbytes(rng.randint(1,32))
 p=OUT/'mutant.spb';p.write_bytes(b);r=call(['check',p]);accepted+=r.get('exit')==0
 if r.get('exit',0)<0 or 'timeout' in r:
  anomalies.append(dict(case=i,result=r,hex=b.hex()));(OUT/f'mutant-failure-{i}.spb').write_bytes(b)
(OUT/'decoder-fuzz.json').write_text(json.dumps(dict(seed=11012026,cases=count,accepted=accepted,anomalies=anomalies),indent=2));print('decoder',count,accepted,len(anomalies),flush=True)
rows=[]
for i in range(200):
 regs=[rng.randint(-10**15,10**15) for _ in range(6)]; code=[f'const r{j}, int {n}' for j,n in enumerate(regs)]
 for _ in range(20):
  op=rng.choice(['add','sub','mul']);a,b,d=[rng.randrange(6) for _ in range(3)]
  if op=='add':v=regs[a]+regs[b]
  elif op=='sub':v=regs[a]-regs[b]
  else:v=regs[a]*regs[b]
  if abs(v).bit_length()>1024:continue
  code.append(f'int.{op} r{d}, r{a}, r{b}');regs[d]=v
 code+=['const r6, int 251','int.mod r0, r0, r6','const r6, int 251','int.add r0, r0, r6','int.mod r0, r0, r6','ret r0']
 expected=regs[0]%251
 src='sparrow 9\nmodule "audit" "v1.0.0"\ntype t0 int\nfunction f0 "main" {\n registers ['+', '.join(['t0']*7)+']\n params []\n result t0\n'+''.join(f' {j}: {s}\n' for j,s in enumerate(code))+'}\nentry f0\n'
 spa=OUT/'generated.spa'; spb=OUT/'generated.spb';spa.write_text(src)
 assembly=call(['--assemble',spa,'-o',spb]); results={}
 if assembly.get('exit')==0:
  for mode in ['off','eager']:
   results[mode]=call([spb],env=dict(os.environ,OKKAMA_JIT=mode))
 if assembly.get('exit')!=0 or any(r.get('exit')!=expected for r in results.values()):
  rows.append(dict(case=i,expected=expected,assembly=assembly,results=results));(OUT/f'generated-failure-{i}.spa').write_text(src)
(OUT/'arithmetic-differential.json').write_text(json.dumps(dict(cases=200,seed=11012026,failures=rows),indent=2));print('arithmetic',200,len(rows),flush=True)
