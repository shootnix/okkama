use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64,Ordering::Relaxed};
static CALLS:AtomicU64=AtomicU64::new(0);
static BYTES:AtomicU64=AtomicU64::new(0);
static LIVE:AtomicU64=AtomicU64::new(0);
static PEAK:AtomicU64=AtomicU64::new(0);
struct Meter;
fn added(n:usize){CALLS.fetch_add(1,Relaxed);BYTES.fetch_add(n as u64,Relaxed);let v=LIVE.fetch_add(n as u64,Relaxed)+n as u64;PEAK.fetch_max(v,Relaxed);}
unsafe impl GlobalAlloc for Meter {
 unsafe fn alloc(&self,l:Layout)->*mut u8 {let p=unsafe{System.alloc(l)};if !p.is_null(){added(l.size())}p}
 unsafe fn alloc_zeroed(&self,l:Layout)->*mut u8 {let p=unsafe{System.alloc_zeroed(l)};if !p.is_null(){added(l.size())}p}
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){LIVE.fetch_sub(l.size() as u64,Relaxed);unsafe{System.dealloc(p,l)}}
 unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8 {let q=unsafe{System.realloc(p,l,n)};if !q.is_null(){LIVE.fetch_sub(l.size() as u64,Relaxed);added(n)}q}
}
#[global_allocator]static ALLOC:Meter=Meter;
fn main(){
 let args:Vec<String>=std::env::args().collect();let bytes=std::fs::read(&args[1]).unwrap();let p=okkama_sparrow::decode(&bytes).unwrap();
 let base=LIVE.load(Relaxed);let calls=CALLS.load(Relaxed);let bytes=BYTES.load(Relaxed);PEAK.store(base,Relaxed);
 let result=sparrow_vm::execute(&p);
 let c=CALLS.load(Relaxed)-calls;let b=BYTES.load(Relaxed)-bytes;let peak=PEAK.load(Relaxed)-base;let retained=LIVE.load(Relaxed) as i128-base as i128;
 eprintln!("AUDIT alloc_calls={c} requested_bytes={b} peak_extra_live_bytes={peak} retained_delta={retained} result={result:?}");
 assert_eq!(result.unwrap().to_i64(),Some(0));
}
