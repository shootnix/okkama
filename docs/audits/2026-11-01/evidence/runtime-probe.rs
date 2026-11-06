
use okkama_runtime_core::{Value, heap::ArrayHeap, native, abi::Native, sched};
use std::{time::{Instant,SystemTime,UNIX_EPOCH},rc::Rc};
fn arr(v:Vec<Value>)->Value{ArrayHeap.allocate(v)}
fn text(s:&str)->Value{Value::Str(s.into())}
fn der(p:&str)->Value{arr(std::fs::read(p).unwrap().into_iter().map(Value::Byte).collect())}
fn main(){
 let a:Vec<String>=std::env::args().collect();
 match a[1].as_str(){
 "cert"=>{
  let n:usize=a[3].parse().unwrap();
  let mut chain=vec![der(&format!("{}/leaf.der",a[2]))];
  for i in 0..n {chain.push(der(&format!("{}/i{}.der",a[2],i)));}
  let args=vec![arr(chain),arr(vec![der(&format!("{}/root.der",a[2]))]),text("DNS"),text("audit.invalid"),text(&SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string()),text("1.3.6.1.5.5.7.3.1")];
  let start=Instant::now();
  let result=native::execute(Native::X509VerifyChain,&args,&mut ArrayHeap,&mut native::Context::default());
  eprintln!("seconds={:.6} result={:?}",start.elapsed().as_secs_f64(),result);
 },
 "registry"=>{
  let n:usize=a[2].parse().unwrap();
  let start=Instant::now();
  let mut last=None;
  for _ in 0..n {let f=sched::resolved(Ok(text("done"))); f.observe(); sched::register(&f); last=Some(Rc::downgrade(&f));}
  let sec=start.elapsed().as_secs_f64();
  let held=last.as_ref().unwrap().upgrade().is_some();
  sched::reset();
  println!("n={n} seconds={sec:.6} retained_after_drop={held} retained_after_reset={}",last.unwrap().upgrade().is_some());
 },
 _=>panic!("mode")
 }
}
