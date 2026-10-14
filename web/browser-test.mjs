// Real browser Worker/Wasm acceptance; no modelled execution or fake timeline.
export async function smoke(workerURL) {
  const worker = new Worker(workerURL, {type:'module'});
  let sequence=0, checks=0, messages=0, maxEvents=0;
  const pending=new Map(), snapshots=new Map(), events=[];
  let holdAck=false;const heldAcks=[];
  let reads=0,yields=0,releasedAtRecord=0,ignored=0;
  worker.onmessage=({data})=>{
    messages++;
    if (data.id!==undefined) {
      const p=pending.get(data.id);if(p){pending.delete(data.id);data.ok?p.resolve(data.value):p.reject(Error(data.error));}
    }
    if (data.type==='update') {
      if(holdAck)heldAcks.push(data.update_id);else worker.postMessage({op:'ack',update_id:data.update_id});
      if(data.snapshot)snapshots.set(data.task,data.snapshot);
      maxEvents=Math.max(maxEvents,data.events?.length??0);
      if(data.ignored)ignored++;
      for(const e of data.events??[]){
        events.push(e);if(events.length>256)events.shift();
        if(e.detail.execution?.read)reads++;
        if(e.kind==='yield')yields++;
        if(e.kind==='record_wait'&&data.snapshot.buffers.length===0)releasedAtRecord++;
      }
    }
  };
  worker.onerror=e=>{for(const p of pending.values())p.reject(Error(e.message));pending.clear();};
  const request=data=>new Promise((resolve,reject)=>{const id=sequence++;pending.set(id,{resolve,reject});worker.postMessage({id,...data});});
  const delay=ms=>new Promise(r=>setTimeout(r,ms));
  const check=(value,message)=>{if(!value)throw Error(message);checks++;};
  async function until(predicate,label){for(let i=0;i<500;i++){if(predicate())return;await delay(10);}throw Error('timeout '+label);}
  async function rejects(data,label){try{await request(data);}catch{checks++;return;}throw Error('expected error '+label);}
  const create=(unit,extra={})=>request({op:'create',unit,...extra});
  try {
    const init=await request({op:'init'});check(init.linear_memory_bytes>0,'memory initialized');
    const first=await request({op:'prepare',source:'x=3\nx=(x+1)*(x-1)+x*2\nprint(x)'});
    check(first.tape.length>3,'actual lowered tape');
    const a=await create(first.unit);
    const step=await request({op:'step',task:a.task});check(step.snapshot.pc===1&&step.events.length===1,'one actual instruction');
    check(step.snapshot.locals[0]==='3','locals from engine');
    await request({op:'run'});await until(()=>snapshots.get(a.task)?.status==='done','scalar');
    check(snapshots.get(a.task).stdout==='14\n','scratch survives step');
    await request({op:'reset'});snapshots.clear();
    const edit=await request({op:'prepare',source:'print(19)'});const b=await create(edit.unit);await request({op:'run'});
    await until(()=>snapshots.get(b.task)?.status==='done','edited source');check(snapshots.get(b.task).stdout==='19\n','source edit changes real output');
    await rejects({op:'prepare',source:'import os'},'unsupported syntax');
    await rejects({op:'prepare',source:'print(missing)'},'unbound input');
    await request({op:'reset'});snapshots.clear();
    const source='quotes=tools.load_quotes(batch)\ntotal=0\nfor q in quotes:\n    total=total+q.unit_price_cents*q.quantity\nreceipt=tools.record_total(batch,total)\nprint(receipt)';
    const prepared=await request({op:'prepare',source,inputs:[['batch','i64']]});
    const ts=[];
    for(const [n,rows] of [10,15,2048].entries()) ts.push(await create(prepared.unit,{inputs:[String(n)],limits:{chunk:32},provider:{rows,delay:50},batch:256}));
    await request({op:'step',task:ts[0].task});check(snapshots.get(ts[0].task).status==='load_wait','actual async wait');
    await until(()=>snapshots.get(ts[0].task)?.status==='ready','provider resume while paused');check(snapshots.get(ts[0].task).status==='ready','provider resume while paused');
    const loaded=snapshots.get(ts[0].task);check(loaded.buffers[0].bytes===160,'real input byte count');
    check(loaded.buffers[0].offset>0&&loaded.frame.offset>0&&loaded.tape.bytes>0,'real offsets');
    await request({op:'run'});await request({op:'pause'});const before=(await request({op:'snapshot',task:ts[0].task})).pc;
    await delay(25);check((await request({op:'snapshot',task:ts[0].task})).pc===before,'pause stops dispatch');
    await request({op:'run'});await until(()=>ts.every(t=>snapshots.get(t.task)?.status==='done'),'multi task');
    for(const [i,t] of ts.entries())check(snapshots.get(t.task).stdout===String([10,15,2048][i]*21)+'\n','independent task result');
    check(reads>=2073&&yields>0,'real row reads and chunk yields');check(releasedAtRecord>=3,'owner released before record wait');
    check(ts.every(t=>snapshots.get(t.task).buffers.length===0),'all owners freed');
    check(maxEvents<=2048&&events.length<=256,'bounded event batch and test ring');
    await request({op:'reset'});snapshots.clear();
    const cunit=await request({op:'prepare',source,inputs:[['batch','i64']]});const c=await create(cunit.unit,{inputs:['2'],provider:{rows:100,delay:50}});
    await request({op:'step',task:c.task});await request({op:'cancel',task:c.task});await until(()=>ignored>0,'late cancelled provider completion');
    check(snapshots.get(c.task).status==='cancelled'&&ignored>0,'cancelled late load discarded');
    check(snapshots.get(c.task).buffers.length===0,'cancel releases allocation');
    await request({op:'reset'});snapshots.clear();
    const dunit=await request({op:'prepare',source,inputs:[['batch','i64']]});
    const failures=[{limits:{allow_load:false}},{limits:{rows:2},provider:{rows:5,delay:0}},{provider:{fail:true,delay:0}},{limits:{stdout_bytes:1},provider:{rows:5,delay:0}}];
    const fs=[];for(const extra of failures)fs.push(await create(dunit.unit,{inputs:['2'],...extra}));await request({op:'run'});
    await until(()=>fs.every(t=>snapshots.get(t.task)?.status==='error'),'errors');for(const t of fs)check(snapshots.get(t.task).buffers.length===0,'errors release owners');
    await request({op:'reset'});snapshots.clear();
    const ov=await request({op:'prepare',source:'print(7)\nprint(9223372036854775807+1)'});const ot=await create(ov.unit,{trace:false});await request({op:'run'});
    await until(()=>snapshots.get(ot.task)?.status==='error','overflow');check(snapshots.get(ot.task).stdout==='7\n'&&snapshots.get(ot.task).error==='Overflow','checked i64 and earlier stdout');
    await rejects({op:'advance',task:ot.task,count:2049},'batch bound');
    await request({op:'reset'});snapshots.clear();
    const big=await request({op:'prepare',source:'print(value)',inputs:[['value','i64']]});const bt=await create(big.unit,{inputs:['9223372036854775807']});await request({op:'run'});
    await until(()=>snapshots.get(bt.task)?.status==='done','bigint');check(snapshots.get(bt.task).stdout==='9223372036854775807\n','lossless i64 strings');
    await request({op:'reset'});
    snapshots.clear();
    const pressure=await request({op:'prepare',source:Array.from({length:10},(_,i)=>`print(${i})`).join('\n')});
    const pt=await create(pressure.unit,{batch:1});holdAck=true;await request({op:'run'});
    await until(()=>heldAcks.length===2,'backpressure');const paused=(await request({op:'snapshot',task:pt.task})).pc;
    await delay(30);check((await request({op:'snapshot',task:pt.task})).pc===paused,'two unacked batches stop dispatch');
    holdAck=false;for(const update_id of heldAcks.splice(0))worker.postMessage({op:'ack',update_id});
    await until(()=>snapshots.get(pt.task)?.status==='done','ack resumes');check(snapshots.get(pt.task).stdout===Array.from({length:10},(_,i)=>String(i)+'\n').join(''),'backpressure resumes exact output');
    await request({op:'reset'});
    return {passed:true,checks,messages,max_event_batch:maxEvents,retained_test_events:events.length,reads,yields,releasedAtRecord,
      memory_bytes:init.linear_memory_bytes};
  } finally {worker.terminate();}
}
