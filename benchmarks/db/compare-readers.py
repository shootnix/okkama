#!/usr/bin/env python3
import os,json,shutil,tempfile,subprocess,statistics,hashlib,argparse
from pathlib import Path
parser=argparse.ArgumentParser(description='Compare DB readers against a git revision, keeping all other stdlib files identical.')
parser.add_argument('--before-ref',required=True)
parser.add_argument('--out',type=Path,required=True)
parser.add_argument('--files',nargs='*',default=['read','row'],help='DB stdlib files restored from before-ref')
parser.add_argument('--before-compiler',type=Path)
parser.add_argument('--engines',default='ir,sparrow,jvm')
args=parser.parse_args()
root=Path(__file__).resolve().parents[2]; out=args.out
out.parent.mkdir(parents=True,exist_ok=True)
tmp=Path(tempfile.mkdtemp(prefix='okkama-db-pair-'))
for version in ['before','after']:
 shutil.copytree(root/'std',tmp/version)
for name in args.files:
 (tmp/'before/db'/(name+'.ok')).write_bytes(subprocess.check_output(['git','show',args.before_ref+':std/db/'+name+'.ok'],cwd=root))
expected=json.loads((root/'benchmarks/db/results/diagnostic.json').read_text())['expected']
digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
result={'method':'5 alternating before/after pairs per engine; sequential processes; 5000 rows, 500 lookups; one internal warmup per phase','before_ref':args.before_ref,'restored_db_files':args.files,'before_compiler_sha256':digest(args.before_compiler) if args.before_compiler else None,'compiler_sha256':digest(root/'okkama-rs/target/release/okkama'),'workload_sha256':digest(root/'benchmarks/db/main.ok'),'native_sources':{str(p.relative_to(root)):digest(p) for p in (root/'okkama-rs/crates/okkama-runtime-core/src/native').glob('*.rs') if p.name in ['db.rs','sqlite.rs']},'scheduler_sha256':digest(root/'okkama-rs/crates/okkama-runtime-core/src/sched.rs'),'expected':expected,'source_hashes':{v:{str(p.relative_to(tmp/v)):digest(p) for p in sorted((tmp/v).rglob('*.ok'))} for v in ['before','after']},'samples':[]}
env={k:v for k,v in os.environ.items() if k not in ('OKKAMA_JIT','OKKAMA_ENGINE','OKKAMA_GC')}
env.update(DB_BENCH_ROWS='5000',DB_BENCH_LOOKUPS='500')
for i in range(5):
 engines=args.engines.split(','); engines=engines[i%len(engines):]+engines[:i%len(engines)]
 for engine in engines:
  for version in (['before','after'] if i%2==0 else ['after','before']):
   env['OKKAMA_STDLIB_DIR']=str(tmp/version)
   compiler=args.before_compiler if version=='before' and args.before_compiler else root/'okkama-rs/target/release/okkama'
   p=subprocess.run([str(compiler),'run','--engine',engine,str(root/'benchmarks/db/main.ok')],env=env,text=True,capture_output=True,timeout=180)
   if p.returncode:raise RuntimeError(p.stdout+p.stderr)
   phases={}
   for line in p.stdout.splitlines():
    name,ns,checksum=line.split();assert int(checksum)==expected[name];phases[name]=int(ns)/1e6
   assert phases.keys()==expected.keys()
   result['samples'].append(dict(iteration=i,engine=engine,version=version,ms=phases))
   out.write_text(json.dumps(result,indent=2)+'\n')
   print(i,engine,version,round(phases['projection_only'],2),flush=True)
result['summary']={e:{phase:{v:statistics.median(s['ms'][phase] for s in result['samples'] if s['engine']==e and s['version']==v) for v in ['before','after']} for phase in expected} for e in args.engines.split(',')}
out.write_text(json.dumps(result,indent=2)+'\n')
shutil.rmtree(tmp)
print(json.dumps(result['summary'],indent=2),flush=True)
