#!/usr/bin/env python3
"""Проверка галереи: анализ всех .ok, оракулы вывода, ошибки ввода и локальные сети.

Запуск: ./examples/gallery/run check [--analysis-only] [--only NAME] [--skip-network].
Использует Python stdlib и установленный Perl 5.42.2. Пишет checks.json и analysis-checks.json.
Все данные dirstat создаются во временном каталоге. Самодостаточные сетевые примеры
выбирают loopback-порты, закрывают серверы и не обращаются к внешним сервисам.
"""
from pathlib import Path
import argparse,collections,hashlib,json,math,os,re,shutil,subprocess,tempfile,time
BASE=Path(__file__).resolve().parent.parent
ROOT=BASE.parent.parent
REPORT=[]

def record(name,condition,detail=''):
    REPORT.append({'name':name,'passed':bool(condition),**({'detail':detail} if detail else {})})
    print(('PASS ' if condition else 'FAIL ')+name,flush=True)
    if not condition:raise AssertionError(name+': '+detail)

def invoke(name,*args,success=True):
    started=time.monotonic()
    result=subprocess.run([str(BASE/'run'),name,*map(str,args)],cwd=ROOT,capture_output=True,timeout=240)
    if success and result.returncode:
        raise AssertionError(name+': '+result.stderr.decode(errors='replace'))
    return result

def perl():
    override=os.environ.get('OKKAMA_GALLERY_ANALYZER_PERL')
    candidates=[override,shutil.which('perl'),str(Path(os.environ.get('PERLBREW_ROOT',str(Path.home()/'perl5/perlbrew')))/'perls'/os.environ.get('OKKAMA_PERL','perl-5.42.2')/'bin/perl')]
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            probe=subprocess.run([candidate,'-e','exit($] >= 5.042002 ? 0 : 1)'],capture_output=True)
            if probe.returncode==0:return candidate
    raise RuntimeError('Select Perl 5.42.2 using OKKAMA_GALLERY_ANALYZER_PERL')

def mandel():
    palette=' .:,;!/> )|&IH%*#'.replace('> ', '>')
    lines=[]
    for y in range(30,0,-1):
        row=[]
        for x in range(75):
            cr=x*.04-2.;ci=y*.1-1.5;zr=zi=0.;k=0
            for k in range(112):
                zr,zi=zr*zr-zi*zi+cr,2.*zr*zi+ci
                if zr*zr+zi*zi>10.:break
            else:k=112
            row.append(palette[k%16])
        lines.append(''.join(row))
    return '\n'.join(lines)+'\n'

def oracle(name,output):
    if name=='quine':return output==(BASE/'quine.ok').read_bytes()
    text=output.decode();lines=text.splitlines()
    if name=='mandel':return text==mandel()
    if name=='brainfuck':return text=='Hello World!\n'
    if name=='quicksort':
        values=[list(map(int,line.split())) for line in lines]
        return len(values)==4 and all(row==sorted(values[0]) for row in values[1:])
    if name=='matrix':return lines==['58 64','139 154','determinant = -10']
    if name=='hanoi':return len(lines)==16 and lines[-1]=='Solved in 15 moves.'
    if name=='nqueens':
        columns=[line.split().index('Q') for line in lines[1:]]
        return lines[0]=='Solutions for N=8: 92' and len(columns)==8 and len(set(columns))==len({r-c for r,c in enumerate(columns)})==len({r+c for r,c in enumerate(columns)})==8
    if name=='fibonacci':
        a,b=0,1;known={}
        for n in range(501):known[n]=a;a,b=b,a+b
        for n,digits,value in re.findall(r'F\((\d+)\) \[(\d+) digits\] = (\d+)',text):
            if int(value)!=known[int(n)] or len(value)!=int(digits):return False
        return str(known[500]) in text
    if name=='pascal':return [list(map(int,line.split())) for line in lines[:12]]==[[math.comb(n,k) for k in range(n+1)] for n in range(12)]
    if name=='sierpinski':
        expected=[]
        for n in range(32):
            row=[' ']*63
            for k in range(n+1):
                if math.comb(n,k)%2:row[31-n+2*k]='#'
            expected.append(''.join(row))
        return lines==expected
    if name=='life':return len(lines)==72 and all(f'generation {n} (5 alive)'==lines[n*9] for n in range(8))
    if name=='cipher':return lines==['Nggnpx ng Qnja! -> Attack at Dawn!','Dwwdfn dw Gdzq! -> Attack at Dawn!','Lxfopv ef Rnhr! -> Attack at Dawn!']
    if name=='anagrams':return lines[0]=='opst  opts post pots spot stop tops' and lines[-1]=='Classes: 10; words: 31'
    if name=='wordcount':return lines[1]=='the 10 ##########' and lines[2]=='fog 6 ######' and len(lines)==11
    if name=='primes':return lines[0]=='2 3 5 7 11 13 17 19 23 29 31 37 41 43 47 53 59 61 67 71' and f'50! = {math.factorial(50)}' in lines and 'Collatz 27: 111' in lines
    if name=='factorize':return '600851475143 = 71 * 839 * 1471 * 6857; divisors = 16' in lines and 'gcd=12 lcm=144' in lines
    if name=='rationals':return 'H_10 = 7381/2520' in lines and '355/113' in lines
    if name=='roman':return '1984 MCMLXXXIV 1984' in lines and '3888 MMMDCCCLXXXVIII 3888' in lines
    if name=='calculator':return [line.rsplit(' = ',1)[-1] for line in lines]==['7','9','5/2','23/2','-11','3/10']
    if name=='rpn':return [line.rsplit(' = ',1)[-1] for line in lines]==['35','9','1/3','17']
    if name=='json-tree':return lines[-1]=='distance exact: 1639/10' and '  null' in lines and '    true' in lines
    if name=='parallel':return lines==['Primes below 1000: 168',*[str(n*n) for n in range(1,13)]]
    if name=='sleep-sort':return len(lines)==2 and sorted(map(int,lines[0].split(':')[1].split()))==sorted(map(int,lines[1].split(':')[1].split()))==list(range(1,10))
    if name=='echo-server':return lines==['hello -> HELLO','okkama sockets -> OKKAMA SOCKETS','goodbye -> GOODBYE','Server stopped cleanly.']
    markers={
      'modules/web/service':['root: 200 Hello from Okkama!','path: 200 Hello, Okkama!'],
      'modules/web/rest-api':['create: 201','delete: 200 deleted','cookie: 200 visit 2','teapot: 418','server error: 500'],
      'modules/web/forms':['urlencoded: 200 order from Ann in teal','multipart: 200 order from Bob in red','upload: 200 Notes: notes.txt, 23 bytes','missing: 422'],
      'modules/web/web-features':['/api/version: 200','/static/nope.css: 404','/events: 200','download: 8192 bytes, identical','middleware saw 12 requests'],
      'modules/web/websocket-chat':['text and binary echo passed','4 ann: hi all','5 room: cat left','7 room: ann left','three clients, ordered JSON and leave events passed'],
    }
    if name in markers:return all(marker in text for marker in markers[name])
    raise AssertionError('Missing oracle: '+name)

def dirstat():
    with tempfile.TemporaryDirectory(prefix='okkama-gallery-') as temporary:
        folder=Path(temporary);(folder/'sub').mkdir();(folder/'.hidden').mkdir()
        for path,text in [('a.txt','abc'),('sub/b.txt','hello'),('stock.csv','x\n'),('LICENSE','yes'),('.secret.txt','hidden'),('.hidden/file.txt','hidden')]:
            (folder/path).write_text(text)
        (folder/'link.txt').symlink_to(folder/'a.txt')
        result=json.loads(invoke('dirstat/dirstat',folder,'--json').stdout)
        record('dirstat totals and hidden/symlink policy',result['total_bytes']==13 and result['kinds']==[{'extension':'txt','files':2,'bytes':8},{'extension':'(none)','files':1,'bytes':3},{'extension':'csv','files':1,'bytes':2}])
        top=json.loads(invoke('dirstat/dirstat',folder,'--top=1','--json').stdout)
        record('dirstat top preserves full total',len(top['kinds'])==1 and top['total_bytes']==13)
        empty=folder/'empty';empty.mkdir();result=json.loads(invoke('dirstat/dirstat',empty,'--json').stdout)
        record('dirstat empty directory',result['total_bytes']==0 and result['kinds']==[])

def main(args):
    result=subprocess.run([perl(),str(BASE/'script/analyze.pl')],cwd=ROOT,capture_output=True,text=True,timeout=180)
    if not result.stdout:raise AssertionError(result.stderr)
    analysis=json.loads(result.stdout);(BASE/'analysis-checks.json').write_text(json.dumps(analysis,ensure_ascii=False,indent=2)+'\n')
    record('syntax and semantic contracts',result.returncode==0,json.dumps([r for r in analysis if not r['passed']],ensure_ascii=False))
    if args.analysis_only:return
    catalog=json.loads((BASE/'catalog.json').read_text())
    for row in catalog['examples']:
        name=row['id']
        if row['mode']=='reject' or name=='dirstat/dirstat':continue
        if args.only and name not in args.only:continue
        if args.skip_network and row['mode']=='network':continue
        result=invoke(name);record(name+' output',oracle(name,result.stdout),result.stdout.decode(errors='replace')[:600])
        record(name+' diagnostics',result.stderr==b'',result.stderr.decode(errors='replace'))
    if not args.only or 'dirstat/dirstat' in args.only:dirstat()
    if not args.only:
        for name,arguments in [('calculator',['1 + )']),('calculator',['1/0']),('rpn',['1 +']),('rpn',['1 2']),('brainfuck',['[']),('brainfuck',['<']),('dirstat/dirstat',['--top=0'])]:
            result=invoke(name,*arguments,success=False);record(name+' rejects '+repr(arguments),result.returncode!=0)
        result=invoke('brainfuck',',.','X');record('Brainfuck input',result.stdout==b'X')
        result=invoke('calculator','0.1 + 0.2');record('calculator decimal exactness',result.stdout.decode().strip()=='0.1 + 0.2 = 3/10')

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--analysis-only',action='store_true');parser.add_argument('--only',action='append');parser.add_argument('--skip-network',action='store_true');args=parser.parse_args()
    known={row['id'] for row in json.loads((BASE/'catalog.json').read_text())['examples'] if row['mode']!='reject'}
    unknown=set(args.only or [])-known
    if unknown:parser.error('Unknown example: '+', '.join(sorted(unknown)))
    try:main(args)
    except Exception as error:
        REPORT.append({'name':'verification exception','passed':False,'detail':str(error)});raise
    finally:
        hashes={str(p.relative_to(BASE)):hashlib.sha256(p.read_bytes()).hexdigest() for p in BASE.rglob('*.ok')}
        (BASE/'checks.json').write_text(json.dumps({'selection':vars(args),'passed':sum(r['passed'] for r in REPORT),'total':len(REPORT),'checks':REPORT,'source_hashes':hashes},ensure_ascii=False,indent=2)+'\n')
    print(f"Checks: {sum(r['passed'] for r in REPORT)}/{len(REPORT)} passed")
