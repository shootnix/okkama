//! Direct rusqlite baseline: no Okkama VM, scheduler or DB object wrappers.
use rusqlite::{Connection, Row, types::Value};
use std::{hint::black_box, time::Instant};

type Snapshot = [Value; 3];
fn snapshot(row: &Row<'_>) -> rusqlite::Result<Snapshot> {
    Ok([row.get(0)?, row.get(1)?, row.get(2)?])
}
fn checksum(row: &Snapshot) -> i64 {
    let Value::Integer(id) = row[0] else { panic!("id") };
    let Value::Text(name) = &row[1] else { panic!("name") };
    let nick = match &row[2] { Value::Null => "guest", Value::Text(v) => v, _ => panic!("nick") };
    id + name.len() as i64 + nick.len() as i64
}
fn one(statement: &mut rusqlite::Statement<'_>, id: i64) -> rusqlite::Result<Snapshot> {
    let mut rows = statement.query([id])?;
    let row = snapshot(rows.next()?.expect("exactly one row"))?;
    assert!(rows.next()?.is_none());
    Ok(row)
}
fn run(db: &Connection, sample: &Snapshot, phase: &str, count: i64, lookups: i64) -> rusqlite::Result<i64> {
    let mut total = 0;
    match phase {
        "projection_only" => {
            for _ in 0..count { total += checksum(black_box(sample)); }
        }
        "scan_raw" | "scan_stream" | "scan_all" => {
            let mut statement = db.prepare_cached("SELECT id,name,nick FROM entries ORDER BY id")?;
            let mut rows = statement.query([])?;
            let mut all = Vec::new();
            while let Some(row) = rows.next()? {
                let owned = snapshot(row)?;
                if phase == "scan_raw" { total += black_box(owned).len() as i64; }
                else if phase == "scan_all" { all.push(owned); }
                else { total += checksum(&owned); }
            }
            if phase == "scan_all" {
                assert!(all.len() <= count as usize);
                for row in &all { total += checksum(row); }
            }
        }
        "lookup_prepared" => {
            let mut statement = db.prepare("SELECT id,name,nick FROM entries WHERE id=?")?;
            for i in 0..lookups { total += checksum(&one(&mut statement, i % count)?); }
        }
        "lookup_statement" => {
            for i in 0..lookups {
                let mut statement = db.prepare_cached("SELECT id,name,nick FROM entries WHERE id=?")?;
                total += checksum(&one(&mut statement, i % count)?);
            }
        }
        "insert_transaction" => {
            db.execute("DELETE FROM inserted", [])?;
            let tx = db.unchecked_transaction()?;
            {
                let mut insert = tx.prepare("INSERT INTO inserted VALUES(?,?)")?;
                for i in 0..lookups { insert.execute((i, "user"))?; }
            }
            tx.commit()?;
            total = db.query_row("SELECT sum(id) AS total FROM inserted", [], |r| r.get(0))?;
        }
        _ => panic!("unknown phase"),
    }
    Ok(total)
}
fn setting(name: &str, default: i64) -> i64 {
    std::env::var(name).map(|v| v.parse().expect("integer setting")).unwrap_or(default)
}
fn main() -> rusqlite::Result<()> {
    let count = setting("DB_BENCH_ROWS", 5000);
    let lookups = setting("DB_BENCH_LOOKUPS", 500);
    let warmups = setting("DB_BENCH_WARMUPS", 4);
    assert!(count > 0 && lookups > 0 && warmups > 0);
    let db = Connection::open_in_memory()?;
    db.set_prepared_statement_cache_capacity(64);
    db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE entries(id INTEGER PRIMARY KEY,name TEXT NOT NULL,nick TEXT); CREATE TABLE inserted(id INTEGER PRIMARY KEY,name TEXT NOT NULL);")?;
    db.execute("WITH RECURSIVE seq(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM seq WHERE i+1<?) INSERT INTO entries SELECT i, 'user-' || i, CASE WHEN i%3=0 THEN NULL WHEN i%5=0 THEN '' ELSE 'nick' END FROM seq", [count])?;
    let sample = db.query_row("SELECT id,name,nick FROM entries WHERE id=0", [], snapshot)?;
    eprintln!("rusqlite=0.37.0 sqlite={}", rusqlite::version());
    for phase in ["projection_only", "scan_raw", "scan_stream", "scan_all", "lookup_prepared", "lookup_statement", "insert_transaction"] {
        for _ in 0..warmups { black_box(run(&db, &sample, phase, count, lookups)?); }
        let start = Instant::now();
        let total = run(&db, &sample, phase, count, lookups)?;
        println!("{phase} {} {total}", start.elapsed().as_nanos());
    }
    Ok(())
}
