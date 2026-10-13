use minipy_native::{buffer::{Quote, QuoteBuffer}, codegen::CodeUnit, provider::Provider, runtime::{self, Cancellation, Limits}};
use std::{rc::Rc, sync::{Arc, Mutex}, time::Instant};
struct Ready(Mutex<Option<QuoteBuffer>>);
impl Provider for Ready {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer,minipy_native::error::TaskError> { Ok(self.0.lock().unwrap().take().unwrap()) }
    async fn record_total(&self, batch: i64, total:i64) -> Result<i64,minipy_native::error::TaskError> { total.checked_add(batch).ok_or(minipy_native::error::TaskError::Overflow) }
}
fn main() {
    let rt=tokio::runtime::Builder::new_current_thread().build().unwrap();
    let source=include_str!("../../fixtures/weighted_scan.py");
    let code=Rc::new(CodeUnit::compile(source).unwrap());
    println!("rows,sample,generation_ns,freeze_ns,execution_cleanup_ns,total_ns,receipt,pointer_preserved,payload_bytes");
    for n in [0,10000,1000000] {
        for sample in 0..25 {
            let start=Instant::now();
            let rows=vec![Quote{unit_price_cents:2,quantity:3};n];
            let generation=start.elapsed().as_nanos();
            let ptr=rows.as_ptr();
            let t=Instant::now();
            let buffer=QuoteBuffer::from_quotes(rows).unwrap();
            let freeze=t.elapsed().as_nanos();
            let preserved=ptr==buffer.rows().as_ptr();
            let payload=buffer.bytes();
            let p=Arc::new(Ready(Mutex::new(Some(buffer))));
            let t=Instant::now();
            let r=rt.block_on(runtime::execute(code.clone(),p,17,700,Limits{chunk:4096,..Limits::default()},Cancellation::default(),None)).unwrap();
            let execution=t.elapsed().as_nanos();
            assert_eq!(r,n as i64*6+17);
            println!("{n},{sample},{generation},{freeze},{execution},{},{r},{preserved},{payload}",start.elapsed().as_nanos());
        }
    }
}
