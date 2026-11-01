use okkama_runtime_core::{Value,IntValue,strings,heap::ArrayHeap};
use std::{hint::black_box,time::Instant};
fn main(){let args:Vec<String>=std::env::args().collect();let n:usize=args[2].parse().unwrap();
 if args[1]=="bytes" {let v=ArrayHeap.allocate(vec![Value::Byte(7);n]);black_box(&v);println!("len={n} slot_bytes={}",std::mem::size_of::<Value>());std::thread::sleep(std::time::Duration::from_millis(5));}
 else if args[1]=="rawbytes"{let v=vec![7u8;n];black_box(&v);println!("len={n}");std::thread::sleep(std::time::Duration::from_millis(5));}
 else {let text=if args[1]=="ascii"{"a".repeat(n)}else{"я".repeat(n)};let mut sum=0;let t=Instant::now();for i in 0..n{sum+=black_box(strings::slice(black_box(&text),&IntValue::from_i64(i as i64),&IntValue::from_i64(i as i64+1)).unwrap()).len();}let a=t.elapsed().as_secs_f64();let t=Instant::now();let one:usize=black_box(&text).chars().map(|ch|black_box(ch).len_utf8()).sum();let b=t.elapsed().as_secs_f64();assert_eq!(sum,one);println!("n={n} repeated_slice={a:.9} iterator={b:.9} sum={sum}");}
}
