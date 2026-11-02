"""Matching SQLite workload; sqlite3.Row and the default statement cache."""
import os
import sqlite3
import time

count = int(os.environ['DB_BENCH_ROWS'])
lookups = int(os.environ['DB_BENCH_LOOKUPS'])
warmups = int(os.environ.get('DB_BENCH_WARMUPS', '1'))
assert warmups > 0
cache_override = os.environ.get('DB_BENCH_SQLITE_CACHE')
cache_options = {} if cache_override is None else {'cached_statements': int(cache_override)}
assert cache_override is None or int(cache_override) >= 0
connection = sqlite3.connect(':memory:', **cache_options)
connection.row_factory = sqlite3.Row
connection.execute('PRAGMA foreign_keys=ON')
connection.execute('CREATE TABLE entries(id INTEGER PRIMARY KEY,name TEXT NOT NULL,nick TEXT)')
connection.execute("WITH RECURSIVE seq(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM seq WHERE i+1<?) INSERT INTO entries SELECT i, 'user-' || i, CASE WHEN i%3=0 THEN NULL WHEN i%5=0 THEN '' ELSE 'nick' END FROM seq", (count,))
connection.execute('CREATE TABLE inserted(id INTEGER PRIMARY KEY,name TEXT NOT NULL)')
connection.commit()
sample = connection.execute("SELECT id,name,nick FROM entries WHERE id=0").fetchone()

def checksum(row):
    return row['id'] + len(row['name']) + len('guest' if row['nick'] is None else row['nick'])

def one(cursor):
    row = cursor.fetchone()
    if row is None or cursor.fetchone() is not None:
        raise AssertionError('expected exactly one row')
    return row

def run(kind):
    total = 0
    if kind == 'projection_only':
        for _ in range(count):
            total += checksum(sample)
    elif kind in ('scan_raw', 'scan_stream', 'scan_all'):
        cursor = connection.execute('SELECT id,name,nick FROM entries ORDER BY id')
        try:
            rows = cursor.fetchall() if kind == 'scan_all' else cursor
            if kind == 'scan_all' and len(rows) > count:
                raise AssertionError('row limit')
            for row in rows:
                total += len(row) if kind == 'scan_raw' else checksum(row)
        finally:
            cursor.close()
    elif kind in ('lookup_prepared', 'lookup_statement'):
        cursor = connection.cursor() if kind == 'lookup_prepared' else None
        try:
            for i in range(lookups):
                active = cursor if cursor is not None else connection.cursor()
                active.execute('SELECT id,name,nick FROM entries WHERE id=?', (i % count,))
                total += checksum(one(active))
                if cursor is None:
                    active.close()
        finally:
            if cursor is not None:
                cursor.close()
    elif kind == 'insert_transaction':
        connection.execute('DELETE FROM inserted')
        connection.commit()
        connection.execute('BEGIN')
        cursor = connection.cursor()
        try:
            for i in range(lookups):
                cursor.execute('INSERT INTO inserted VALUES(?,?)', (i, 'user'))
            connection.commit()
        except BaseException:
            connection.rollback()
            raise
        finally:
            cursor.close()
        cursor = connection.execute('SELECT sum(id) AS total FROM inserted')
        total = one(cursor)['total']
        cursor.close()
    return total

try:
    for kind in ('projection_only', 'scan_raw', 'scan_stream', 'scan_all', 'lookup_prepared', 'lookup_statement', 'insert_transaction'):
        for _ in range(warmups):
            run(kind)
        start = time.perf_counter_ns()
        value = run(kind)
        print(kind, time.perf_counter_ns() - start, value)
finally:
    connection.close()
