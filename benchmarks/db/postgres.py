"""The DB benchmark workload on PostgreSQL through psycopg2 (libpq), dict rows."""
import os
import psycopg2
import psycopg2.extras
import time

count = int(os.environ['DB_BENCH_ROWS'])
lookups = int(os.environ['DB_BENCH_LOOKUPS'])
warmups = int(os.environ.get('DB_BENCH_WARMUPS', '1'))
assert warmups > 0
connection = psycopg2.connect(host=os.environ.get('OKKAMA_PG_ADDRESS', '127.0.0.1'), user='postgres',
                              password=os.environ.get('OKKAMA_PG_PASSWORD', ''), dbname='postgres',
                              cursor_factory=psycopg2.extras.DictCursor)
setup = connection.cursor()
setup.execute('CREATE TEMP TABLE entries(id bigint PRIMARY KEY,name text NOT NULL,nick text)')
setup.execute("INSERT INTO entries SELECT i, 'user-' || i, CASE WHEN i%%3=0 THEN NULL WHEN i%%5=0 THEN '' ELSE 'nick' END FROM generate_series(0,%s-1) AS i", (count,))
setup.execute('CREATE TEMP TABLE inserted(id bigint PRIMARY KEY,name text NOT NULL)')
connection.commit()
setup.execute("SELECT id,name,nick FROM entries WHERE id=0")
sample = setup.fetchone()

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
        cursor = connection.cursor()
        cursor.execute('SELECT id,name,nick FROM entries ORDER BY id')
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
                active.execute('SELECT id,name,nick FROM entries WHERE id=%s', (i % count,))
                total += checksum(one(active))
                if cursor is None:
                    active.close()
        finally:
            if cursor is not None:
                cursor.close()
    elif kind == 'insert_transaction':
        cursor = connection.cursor()
        cursor.execute('DELETE FROM inserted')
        connection.commit()
        try:
            for i in range(lookups):
                cursor.execute('INSERT INTO inserted VALUES(%s,%s)', (i, 'user'))
            connection.commit()
        except BaseException:
            connection.rollback()
            raise
        finally:
            cursor.close()
        cursor = connection.cursor()
        cursor.execute('SELECT sum(id)::bigint AS total FROM inserted')
        total = int(one(cursor)['total'])
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
