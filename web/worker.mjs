import {Engine} from './engine.mjs';
let engine, loading, running = false, epoch = 0, cursor = 0, tickPending = false;
const tasks = new Map();
const outstanding = new Set();
let updateSequence = 0;
let replacing = false;
const timers = new Set();
function later(fn, delay) {
  const timer = setTimeout(() => { timers.delete(timer); fn(); }, delay);
  timers.add(timer);
}
function clearSession() {
  running = false; epoch++; cursor = 0; tickPending = false;
  for (const timer of timers) clearTimeout(timer);
  timers.clear(); outstanding.clear(); tasks.clear();
}
function providerConfig(data) {
  const p = data.provider ?? {};
  const rows = p.rows ?? 512, delay = p.delay ?? 25, batch = data.batch ?? 1024;
  if (!Number.isInteger(rows)||rows<0||rows>1_000_000||!Number.isFinite(delay)||delay<0||delay>5000||!Number.isInteger(batch)||batch<1||batch>2048) throw new Error('invalid bounded provider/batch options');
  const price = String(p.price ?? 7), quantity = String(p.quantity ?? 3);
  for (const v of [price, quantity]) if (!/^-?\d+$/.test(v)||BigInt(v)<-(1n<<63n)||BigInt(v)>=(1n<<63n)) throw new Error('invalid provider i64');
  return {rows, delay, price, quantity, fail: !!p.fail, batch, trace: data.trace ?? true};
}
const emit = value => {
  const update_id = ++updateSequence;
  outstanding.add(update_id);
  postMessage({type:'update', epoch, update_id, ...value});
};
const terminal = s => ['done','error','cancelled'].includes(s);
function accept(value) {
  if (value.snapshot && value.task !== undefined) tasks.get(value.task).status = value.snapshot.status;
  emit(value);
  for (const event of value.events ?? []) {
    if (!['load_wait','record_wait'].includes(event.kind)) continue;
    const task = event.task, config = tasks.get(task), generation = epoch;
    const tool = event.detail.tool;
    // This timer is a detached trusted provider operation. Cancellation stops
    // the consumer, not an already launched effect. Reset discards stale delivery.
    later(() => {
      if (generation !== epoch) return;
      const command = {op:'complete', task, request:tool.request};
      if (config.fail) command.error = 'fixture provider failure';
      else if (event.kind === 'load_wait') Object.assign(command,
        {rows:config.rows, price:config.price, quantity:config.quantity});
      else command.receipt = tool.total;
      try { accept({task, ...engine.command(command)}); schedule(); }
      catch (error) { postMessage({type:'provider_error',task,error:String(error)}); }
    }, config.delay);
  }
}
function schedule() {
  if (!running || tickPending || outstanding.size >= 2) return;
  tickPending = true;
  later(() => {
    tickPending = false;
    if (!running || outstanding.size >= 2) return;
    const ready = [...tasks].filter(([,t]) => t.status === 'ready');
    if (!ready.length) {
      if ([...tasks.values()].every(t => terminal(t.status))) running = false;
      return;
    }
    const [task, config] = ready[cursor++ % ready.length];
    accept({task,...engine.command({op:'advance',task,count:config.batch,trace:config.trace})});
    schedule();
  }, 0);
}
onmessage = async ({data}) => {
  const {id, op} = data;
  try {
    if (op === 'ack') {outstanding.delete(data.update_id);schedule();return;}
    if (op === 'init') {
      loading ??= Engine.load(data.url ? new URL(data.url, self.location.href) : undefined);
      engine = await loading;
      loading = undefined;
      postMessage({id,ok:true,value:{initialized:true,linear_memory_bytes:engine.exports.memory.buffer.byteLength,
        timing:{initialization_ms:engine.initialization_ms}}});
      return;
    }
    if (!engine) throw new Error('init required');
    let value;
    if (op === 'run') {running=true;schedule();value={running};}
    else if (op === 'pause') {running=false;value={running};}
    else if (op === 'reset') {value=engine.command({op:'reset'});clearSession();value.epoch=epoch;}
    else if (op === 'recompile') {
      if (replacing) throw new Error('recompile already in progress');
      replacing = true;
      const generation = epoch;
      try {
        const config = providerConfig(data);
        // Reuse the compiled Wasm module, but isolate candidate memory/state.
        // Neither prepare nor create failure touches the current session.
        const candidate = await engine.fork();
        const prepared = candidate.command({op:'prepare', source:data.source, inputs:data.input_types ?? []});
        const created = candidate.command({op:'create', unit:prepared.unit, inputs:data.inputs ?? [], limits:data.limits ?? {}});
        if (generation !== epoch) throw new Error('session changed during recompile');
        clearSession();
        engine = candidate;
        tasks.set(created.task, {...config, status:created.snapshot.status});
        value = {...prepared, ...created, epoch, timing:prepared.timing};
      } finally { replacing = false; }
    }
    else if (op === 'create') {
      const config=providerConfig(data);
      value=engine.command({op:'create',unit:data.unit,inputs:data.inputs??[],limits:data.limits??{}});
      tasks.set(value.task,{...config,status:value.snapshot.status});
    } else if (op === 'step' || op === 'advance') {
      if (outstanding.size >= 2) throw new Error('ack outstanding updates before stepping');
      running=false;
      value={task:data.task,...engine.command({op:'advance',task:data.task,count:op==='step'?1:data.count,trace:true})};
      accept(value);
    } else if (op === 'cancel') {
      if (outstanding.size >= 2) throw new Error('ack outstanding updates before cancellation');
      value={task:data.task,...engine.command({op:'cancel',task:data.task})};accept(value);
    } else if (op === 'prepare' || op === 'snapshot') value=engine.command(data);
    else throw new Error('unknown worker operation');
    postMessage({id,ok:true,value});
  } catch (error) {postMessage({id,ok:false,error:String(error)});}
};
