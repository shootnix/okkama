#!/usr/bin/env python3
"""Measure statement-cache benefit and verify actual preparation, including Python 3.9's minimum cache size."""
import hashlib
import json
from pathlib import Path
import sqlite3
import statistics
import sys
import time

HERE = Path(__file__).resolve().parent
MODES = ('default', 'requested_zero', 'unique_sql')
SQL = 'SELECT id,name,nick FROM entries WHERE id=?'
ITERATIONS = 500
WARMUPS = 4


def connection(mode):
    db = sqlite3.connect(':memory:', **({} if mode == 'default' else {'cached_statements': 0}))
    db.row_factory = sqlite3.Row
    db.execute('CREATE TABLE entries(id INTEGER PRIMARY KEY,name TEXT NOT NULL,nick TEXT)')
    db.execute("WITH RECURSIVE seq(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM seq WHERE i+1<5000) INSERT INTO entries SELECT i,'user-'||i,CASE WHEN i%3=0 THEN NULL WHEN i%5=0 THEN '' ELSE 'nick' END FROM seq")
    return db


def queries(mode, tag):
    return [SQL+f' /* {tag}:{i} */' if mode == 'unique_sql' else SQL for i in range(ITERATIONS)]


def run(db, texts):
    cursor = db.cursor()
    total = 0
    try:
        for i, sql in enumerate(texts):
            cursor.execute(sql, (i,))
            row = cursor.fetchone()
            if row is None or cursor.fetchone() is not None:
                raise AssertionError('cardinality')
            total += row['id']+len(row['name'])+len('guest' if row['nick'] is None else row['nick'])
    finally:
        cursor.close()
    return total


def main():
    expected = sum(i+len('user-'+str(i))+(5 if i%3==0 else 0 if i%5==0 else 4) for i in range(ITERATIONS))
    preparations = {}
    for mode in MODES:
        db = connection(mode)
        count = [0]
        def authorizer(action, *unused):
            if action == sqlite3.SQLITE_SELECT:
                count[0] += 1
            return sqlite3.SQLITE_OK
        db.set_authorizer(authorizer)
        assert run(db, queries(mode, 'probe')) == expected
        preparations[mode] = count[0]
        db.close()
    assert preparations['unique_sql'] == ITERATIONS
    assert preparations['default'] == 1
    # Timing deliberately excludes authorizer callbacks and query-string creation.
    samples = []
    for iteration in range(5):
        modes = MODES[iteration%3:]+MODES[:iteration%3]
        for mode in modes:
            db = connection(mode)
            batches = [queries(mode, f'{iteration}:{j}') for j in range(WARMUPS+1)]
            for batch in batches[:-1]:
                assert run(db, batch) == expected
            start = time.perf_counter_ns()
            total = run(db, batches[-1])
            elapsed = time.perf_counter_ns()-start
            assert total == expected
            db.close()
            samples.append(dict(iteration=iteration, mode=mode, ms=elapsed/1e6, checksum=total))
    result = dict(python=sys.version, sqlite=sqlite3.sqlite_version, iterations=ITERATIONS,
                  warmups=WARMUPS, preparation_counts=preparations, samples=samples,
                  summary={m:statistics.median(s['ms'] for s in samples if s['mode']==m) for m in MODES},
                  source_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  method='Same 500 keyed reads and reused cursor. Unique SQL differs only by prebuilt comments, forcing Python statement-cache misses. This control includes miss bookkeeping; it does not measure preparation alone. Counts measured separately with SQLITE_SELECT authorizer events; no callbacks in timing.')
    (HERE/'results/python-cache-probe.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))


if __name__ == '__main__':
    main()
