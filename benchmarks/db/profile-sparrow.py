"""macOS sample of standalone prepared-profile SPB after its warmup marker."""
import argparse, hashlib, json, os, subprocess
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--vm',required=True,type=Path)
p.add_argument('--bytecode',required=True,type=Path)
p.add_argument('--out',required=True,type=Path,help='Output prefix')
a=p.parse_args()
env=dict(os.environ,DB_BENCH_ROWS='5000',DB_BENCH_LOOKUPS='300000',DB_BENCH_WARMUPS='4',OKKAMA_JIT_SCALAR='on')
for k in ('OKKAMA_JIT','OKKAMA_JIT_DUMP','OKKAMA_JIT_ONLY','OKKAMA_JIT_SKIP','OKKAMA_NO_REGIONS_BENCH','OKKAMA_SQLITE_INLINE_BENCH'):
    env.pop(k,None)
child=subprocess.Popen([str(a.vm),str(a.bytecode)],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
try:
    ready=child.stdout.readline()
    assert ready.strip()=='PROFILE_READY',ready
    profile=subprocess.run(['/usr/bin/sample',str(child.pid),'8','1','-file',str(a.out)+'.txt'],text=True,capture_output=True)
    if profile.returncode:
        raise RuntimeError(profile.stderr)
    out,err=child.communicate(timeout=180)
    assert child.returncode==0,(out,err)
    phase,ns,checksum=out.split()
    expected=sum(i+len('user-'+str(i))+(5 if i%3==0 else 0 if i%5==0 else 4) for i in range(5000))*60
    assert phase=='lookup_prepared' and int(checksum)==expected
    result={'method':'8 second stack sampling after four warmups; 300000 lookups over 5000 rows; worker enabled. Profiled duration is not a benchmark timing.','lookups':300000,'rows':5000,'stdout':ready+out,'stderr':err,'sample_exit':profile.returncode,'sha256':{str(x):hashlib.sha256(x.read_bytes()).hexdigest() for x in (a.vm,a.bytecode)}}
    Path(str(a.out)+'.json').write_text(json.dumps(result,indent=2)+'\n')
finally:
    if child.poll() is None:
        child.terminate();child.communicate(timeout=10)
