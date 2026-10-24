#!/usr/bin/env python3
"""Карта реальных импортов приложений, помощников, labs и std.

Запуск: ./examples/cookbook/run coverage; python3 examples/cookbook/script/coverage.py --write
Вход: текущие .ok. Выход --write: coverage.json и COVERAGE.md; --check сверяет карту.
Карта отражает зависимости исходников; результаты выполнения записывает verify.py.
"""
from pathlib import Path
from collections import deque
import argparse,json,re,hashlib
BASE=Path(__file__).resolve().parent.parent
ROOT=BASE.parent.parent
STD=ROOT/'std'

def inventory():
    modules={path.resolve():str(path.relative_to(STD))[:-3] for path in STD.rglob('*.ok')}
    graph={}
    def load(path):
        if path in graph:return
        dependencies=[]
        for name in re.findall(r'^import\s+"([^"]+)"',path.read_text(),re.M):
            found=next((candidate.resolve() for candidate in (path.parent/(name+'.ok'),STD/(name+'.ok')) if candidate.is_file()),None)
            if found is None:raise ValueError(f'Unresolved {name}: {path}')
            dependencies.append(found)
        graph[path]=dependencies
        for dependency in dependencies:load(dependency)
    entries=sorted((BASE/'apps').glob('*.ok'))+sorted((BASE/'labs').glob('*/main.ok'))+sorted((BASE/'web').glob('*/main.ok'))
    coverage={}
    def label(path):
        return 'std/'+modules[path]+'.ok' if path in modules else str(path.relative_to(BASE))
    for entry in entries:
        entry=entry.resolve();load(entry);pending=deque([(entry,[label(entry)])]);seen=set()
        while pending:
            path,chain=pending.popleft()
            if path in seen:continue
            seen.add(path)
            if path in modules:
                module=modules[path];record={'entry':label(entry),'chain':chain,'direct':len(chain)==2}
                previous=coverage.get(module)
                score=lambda r:(not r['entry'].startswith('labs/'),-len(r['chain']))
                if previous is None or score(record)>score(previous):coverage[module]=record
            for dependency in graph[path]:pending.append((dependency,chain+[label(dependency)]))
    return {'total':len(modules),'covered':len(coverage),'entries':len(entries),
            'modules':dict(sorted(coverage.items())),
            'source_hashes':{label(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(graph)},
            'unassigned':sorted(set(modules.values())-coverage.keys())}

def markdown(report):
    rows=['# Карта использования stdlib','',
      f"{report['covered']} из {report['total']} модулей связаны с {report['entries']} точками запуска.",
      '', 'Карта описывает прямые импорты и транзитивные зависимости реальных исходников. Цепочка включает вспомогательные модули приложения. Для internal и runtime это путь через использующий их API. Результаты выполнения сценариев хранятся отдельно в [checks.json](checks.json).', '',
      '| Модуль | Приложение или лаборатория | Цепочка импортов |','|---|---|---|']
    for module,record in report['modules'].items():
        chain=' → '.join('`'+part.removesuffix('.ok')+'`' for part in record['chain'])
        rows.append(f"| [`{module}`](../../std/{module}.ok) | [{record['entry']}]({record['entry']}) | {chain} |")
    rows+=['','Повторить проверку: `./examples/cookbook/run coverage`. Обновить карту после изменения исходников: `python3 examples/cookbook/script/coverage.py --write`.','']
    return '\n'.join(rows)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');parser.add_argument('--write',action='store_true');args=parser.parse_args()
    report=inventory()
    if report['unassigned']:raise SystemExit('Unassigned modules: '+', '.join(report['unassigned']))
    if args.write:
        (BASE/'coverage.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
        (BASE/'COVERAGE.md').write_text(markdown(report))
    if args.check:
        saved=json.loads((BASE/'coverage.json').read_text())
        if saved!=report:raise SystemExit('Coverage changed; run coverage.py --write')
    print(f"Coverage: {report['covered']}/{report['total']} modules, {report['entries']} programs")
if __name__=='__main__':main()
