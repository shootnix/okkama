#!/usr/bin/env python3
"""Проверки настоящих CLI, SQLite, HTTP и WebSocket приложений.

Запуск: ./examples/cookbook/run verify
Данные создаются во временном каталоге; серверы слушают свободные loopback-порты.
Результат: checks.json, строки PASS и код завершения. Внешние аккаунты не используются.
"""
from pathlib import Path
from contextlib import contextmanager
import base64,hashlib,json,os,select,signal,socket,struct,subprocess,tempfile,time,urllib.request,urllib.error
BASE=Path(__file__).resolve().parent.parent
ROOT=BASE.parent.parent
RUN=BASE/'run'
RESULTS=[]

def check(label,condition):
    RESULTS.append({'name':label,'passed':bool(condition)})
    if not condition:raise AssertionError(label)
    print('PASS',label,flush=True)

def command(app,*args,success=True):
    result=subprocess.run([str(RUN),app,*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=60)
    if success and result.returncode:raise AssertionError(f'{app} {args}:\n{result.stdout}\n{result.stderr}')
    if not success:check('rejected '+app,result.returncode!=0)
    return result

def request(port,path='/',body=None):
    raw=None if body is None else json.dumps(body,ensure_ascii=False).encode()
    req=urllib.request.Request(f'http://127.0.0.1:{port}{path}',data=raw,headers={'Content-Type':'application/json'} if body is not None else {})
    try:
        with urllib.request.urlopen(req,timeout=60) as response:return response.status,response.read().decode()
    except urllib.error.HTTPError as response:return response.code,response.read().decode()

@contextmanager
def server(app,*args):
    errors=tempfile.TemporaryFile(mode='w+t')
    process=subprocess.Popen([str(RUN),app,*map(str,args),"0"],cwd=ROOT,stdout=subprocess.PIPE,stderr=errors,text=True,start_new_session=True)
    try:
        ready,_,_=select.select([process.stdout],[],[],45)
        line=process.stdout.readline().strip() if ready else ''
        if not line.startswith('PORT '):
            errors.seek(0);raise AssertionError(f'{app} startup: {line}\n{errors.read()}')
        yield int(line.split()[1])
    finally:
        if process.poll() is None:
            os.killpg(process.pid,signal.SIGTERM)
        try:process.wait(timeout=10)
        except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
        process.stdout.close();errors.close()

class WebSocket:
    def __init__(self,port,origin=None):
        self.socket=socket.create_connection(('127.0.0.1',port),timeout=15)
        self.socket.settimeout(15);self.pending=b''
        key=base64.b64encode(os.urandom(16)).decode()
        self.socket.sendall((f'GET /socket HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: okkama-chat\r\nOrigin: {origin or f"http://127.0.0.1:{port}"}\r\n\r\n').encode())
        wire=b''
        while b'\r\n\r\n' not in wire:
            chunk=self.socket.recv(4096)
            if not chunk:
                self.socket.close();raise ValueError('Server closed before WebSocket upgrade')
            wire+=chunk
        head,self.pending=wire.split(b'\r\n\r\n',1)
        if b' 101 ' not in head:
            self.socket.close();raise ValueError(head.decode())
        expected=base64.b64encode(hashlib.sha1((key+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest())
        assert expected.lower() in head.lower()
    def read(self,n):
        while len(self.pending)<n:
            chunk=self.socket.recv(65536)
            if not chunk:raise EOFError()
            self.pending+=chunk
        result,self.pending=self.pending[:n],self.pending[n:];return result
    def send(self,text,opcode=1):
        payload=text.encode() if isinstance(text,str) else text
        mask=os.urandom(4);length=len(payload)
        header=bytes([0x80|opcode,0x80|length]) if length<126 else bytes([0x80|opcode,0x80|126])+struct.pack('!H',length)
        masked=bytes(value^mask[index%4] for index,value in enumerate(payload))
        self.socket.sendall(header+mask+masked)
    def message(self):
        while True:
            first,second=self.read(2);length=second&127
            if length==126:length=struct.unpack('!H',self.read(2))[0]
            elif length==127:length=struct.unpack('!Q',self.read(8))[0]
            mask=self.read(4) if second&128 else None
            payload=self.read(length)
            if mask:payload=bytes(v^mask[i%4] for i,v in enumerate(payload))
            kind=first&15
            if kind==9:self.send(payload,10);continue
            if kind==8:return None
            if kind==1:return payload.decode()
    def close(self):
        try:self.send(struct.pack('!H',1000),8)
        finally:self.socket.close()

def main():
    with tempfile.TemporaryDirectory(prefix='okkama-cookbook-') as work:
        folder=Path(work)
        payroll=json.loads(command('payroll','demo').stdout)
        check('payroll exact totals',[(r['employee'],r['minutes'],r['amount_cents']) for r in payroll]==[('Ада',570,'23750'),('Барак',300,'15000')])
        day=json.loads(command('payroll','report','examples/cookbook/data/shifts.csv','Ада','2026-10-18').stdout)
        check('payroll day filter',day[0]['amount_cents']=='3750')
        ledger=folder/'shifts.csv'
        command('payroll','init',ledger)
        command('payroll','add',ledger,'Ада','2026-10-18',90,2500)
        before=ledger.read_bytes()
        command('payroll','init',ledger,success=False)
        check('payroll preserves existing history',ledger.read_bytes()==before)
        daily=json.loads(command('payroll','daily',ledger).stdout)
        shift=daily[0]['employees'][0]
        check('payroll daily hours',daily[0]['day']=='2026-10-18' and shift['hours']==1 and shift['remaining_minutes']==30 and shift['amount_cents']=='3750')
        expected=json.loads((BASE/'data/stock.json').read_text())
        for source in ('json','csv','toml'):
            for destination in ('json','csv','toml'):
                rendered=command('formats','convert',source,destination,f'examples/cookbook/data/stock.{source}').stdout
                target=folder/f'roundtrip-{source}.{destination}';target.write_text(rendered)
                restored=json.loads(command('formats','convert',destination,'json',target).stdout)
                check(f'format roundtrip {source}/{destination}',restored==expected)
        eml=folder/'message.eml'
        command('mail','compose',eml,'reader@example.test','Привет','Текст письма')
        check('mail Unicode MIME roundtrip','Текст письма' in command('mail','read',eml).stdout)
        command('mail','compose',folder/'bad.eml','reader@example.test','evil\r\nBcc: injected@example.test','text',success=False)
        taxi=folder/'taxi.sqlite';command('taxi','init',taxi);command('taxi','driver-add',taxi,'Ада')
        command('taxi','trip-add',taxi,1,'2026-10-18',30,1200)
        report=json.loads(command('taxi','report',taxi).stdout)
        check('taxi persists parameterized trip',report[0]['trips']==1 and report[0]['fare_cents']==1200)
        command('taxi','trip-add',taxi,999,'2026-10-18',30,100,success=False)
        check('taxi rejects missing driver',json.loads(command('taxi','report',taxi).stdout)==report)
        shop=folder/'shop.sqlite'
        with server('web-bookstore',shop) as port:
            code,page=request(port);check('bookstore HTML',code==200 and 'Книжный магазин' in page)
            code,receipt=request(port,'/api/orders',{'book':1,'quantity':2})
            check('bookstore HTTP purchase',code==201 and json.loads(receipt)['total_cents']==6400)
            code,_=request(port,'/api/orders',{'book':1,'quantity':999});check('bookstore stock rejection',code==409)
            code,books=request(port,'/api/books');check('bookstore rollback preserves stock',json.loads(books)[0]['stock']==3)
            code,orders=request(port,'/api/orders');check('bookstore immutable order history',len(json.loads(orders))==1)
        blog=folder/'blog.sqlite'
        with server('web-blog',blog) as port:
            code,_=request(port,'/api/posts',{'title':'Первая запись','body':'<script>alert(1)</script>'});check('blog HTTP publish',code==201)
            code,posts=request(port,'/api/posts');post=json.loads(posts)[0]
            code,page=request(port,f"/posts/{post['id']}");check('blog escapes user HTML',code==200 and '&lt;script&gt;' in page and '<script>alert(1)</script>' not in page)
            code,_=request(port,'/api/posts',{'title':'','body':'text'});check('blog rejects empty title',code==422)
        check('blog persists across process',len(json.loads(command('blog','list',blog).stdout))==1)
        with server('docs') as port:
            for path,label in [('/','Документация Okkama'),('/language','Okkama'),('/stdlib','стандартная библиотека'),('/cookbook','Учебные приложения')]:
                code,page=request(port,path);check('documentation '+path,code==200 and label.lower() in page.lower())
            code,page=request(port,'/stdlib?page=1');check('documentation pagination',code==200 and 'Назад' in page)
            code,_=request(port,'/stdlib?page=bad');check('documentation invalid page',code==400)
        with server('chat') as port:
            one=WebSocket(port);two=WebSocket(port)
            try:
                check('chat welcomes both clients','Добро пожаловать' in one.message() and 'Добро пожаловать' in two.message())
                one.send('Ада: привет');check('chat broadcasts to sender',one.message()=='Ада: привет');check('chat broadcasts to peer',two.message()=='Ада: привет')
                two.send('Барак: ответ');check('chat reverse broadcast',one.message()=='Барак: ответ' and two.message()=='Барак: ответ')
                cli=command('chat-client',port,'CLI: привет')
                check('chat Okkama CLI client','Добро пожаловать' in cli.stdout and 'CLI: привет' in cli.stdout)
            finally:one.close();two.close()
            try:WebSocket(port,'http://foreign.example');allowed=True
            except ValueError:allowed=False
            check('chat checks Origin',not allowed)
        tests=command('tests','--report=json');wire=json.loads(tests.stdout)
        check('Okkama test framework',all(row['status']=='passed' for row in wire['results']))
    check('stdlib coverage',command('coverage').returncode==0)

if __name__=='__main__':
    try:
        main()
    except Exception as error:
        RESULTS.append({'name':'verification exception','passed':False,'error':str(error)})
        raise
    finally:
        (BASE/'checks.json').write_text(json.dumps({'checks':RESULTS,'passed':sum(r['passed'] for r in RESULTS),'total':len(RESULTS)},ensure_ascii=False,indent=2)+'\n')
    print('Application checks:',len(RESULTS),'passed')
