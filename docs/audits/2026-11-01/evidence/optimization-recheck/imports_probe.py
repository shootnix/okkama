from pathlib import Path
import subprocess,time,json,os,statistics
repo=Path('/Volumes/SamsungSSD/stuff/okkama-new');out=Path('/tmp/okkama-opt-recheck-20261101');rows=[]
for n in [16,64,256]:
 for variant in ['once','repeated']:
  d=out/f'imports-{n}-{variant}';d.mkdir(exist_ok=True)
  (d/'shared.ok').write_text('module shared v1.0.0;\n'+''.join(f'object Item{i} {{ Item{i}(); }}\n' for i in range(2000)))
  (d/'main.ok').write_text('module main v1.0.0;\nimport "shared";\n'+''.join(f'import "m{i}";\n' for i in range(n))+'main() is int { 0; }\n')
  for i in range(n):(d/f'm{i}.ok').write_text(f'module m{i} v1.0.0;\n'+('import "shared";\n' if variant=='repeated' else ''))
 for repeat in range(4):
  for variant in (['once','repeated'] if repeat%2==0 else ['repeated','once']):
   d=out/f'imports-{n}-{variant}';t=time.perf_counter();p=subprocess.Popen([str(repo/'okkama-rs/target/release/okkama'),'check',str(d/'main.ok')],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
   _,status,ru=os.wait4(p.pid,0);p.returncode=os.waitstatus_to_exitcode(status);stdout,stderr=p.communicate();dt=time.perf_counter()-t
   assert p.returncode==0,stderr
   if repeat:rows.append(dict(n=n,variant=variant,repeat=repeat,seconds=dt,rss_bytes=ru.ru_maxrss))
 for variant in ['once','repeated']:print(n,variant,statistics.median(r['seconds'] for r in rows if r['n']==n and r['variant']==variant),flush=True)
(out/'import-reparse.json').write_text(json.dumps(rows,indent=2))
