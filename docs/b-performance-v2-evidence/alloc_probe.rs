use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use minipy_native::buffer::{Quote,QuoteBuffer};
static CURRENT:AtomicUsize=AtomicUsize::new(0);
static PEAK:AtomicUsize=AtomicUsize::new(0);
static ALLOCS:AtomicUsize=AtomicUsize::new(0);
struct Count;
unsafe impl GlobalAlloc for Count {
 unsafe fn alloc(&self,l:Layout)->*mut u8 { let p=unsafe{System.alloc(l)}; if !p.is_null(){ALLOCS.fetch_add(1,Relaxed);let n=CURRENT.fetch_add(l.size(),Relaxed)+l.size();PEAK.fetch_max(n,Relaxed);} p }
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){CURRENT.fetch_sub(l.size(),Relaxed);unsafe{System.dealloc(p,l)}}
 unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8 {let q=unsafe{System.realloc(p,l,n)};if !q.is_null(){ALLOCS.fetch_add(1,Relaxed);let old=CURRENT.fetch_sub(l.size(),Relaxed);let now=old-l.size()+n;CURRENT.fetch_add(n,Relaxed);PEAK.fetch_max(now,Relaxed);}q}
}
#[global_allocator] static A:Count=Count;
fn main(){
 println!("rows,input_capacity_bytes,freeze_allocations,freeze_peak_extra_bytes,frozen_extra_bytes,weak_only_extra_bytes,pointer_preserved");
 for n in [0,10000,1000000]{
  let base=CURRENT.load(Relaxed);
  let v=vec![Quote{unit_price_cents:2,quantity:3};n];
  let capacity=v.capacity()*std::mem::size_of::<Quote>();let ptr=v.as_ptr();
  PEAK.store(CURRENT.load(Relaxed),Relaxed);let ac=ALLOCS.load(Relaxed);
  let b=QuoteBuffer::from_quotes(v).unwrap();
  let allocations=ALLOCS.load(Relaxed)-ac;
  let peak=PEAK.load(Relaxed)-base;
  let frozen=CURRENT.load(Relaxed)-base;
  let preserved=ptr==b.rows().as_ptr();
  let weak=b.downgrade();drop(b);
  let weak_only=CURRENT.load(Relaxed)-base;
  assert!(weak.upgrade().is_none());drop(weak);
  assert_eq!(CURRENT.load(Relaxed),base);
  println!("{n},{capacity},{allocations},{peak},{frozen},{weak_only},{preserved}");
 }
}
