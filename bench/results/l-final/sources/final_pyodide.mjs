// L: one resident Node/Pyodide instance. Node CPU includes WASM and JS host.
// argv: RUNTIME_DIR OUT_DIR round|memory|init [round_id|unique|shared]
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL, fileURLToPath} from 'node:url';
import {gzipSync} from 'node:zlib';
const [runtime, output, mode, arg] = process.argv.slice(2);
const OUT = path.resolve(output);
fs.mkdirSync(OUT, {recursive: true});
const cpu = () => { const c = process.cpuUsage(); return (c.user + c.system) * 1000; };
const physical = () => {
  const t = fs.readFileSync('/proc/self/smaps_rollup', 'utf8');
  const get = k => Number(t.match(new RegExp('^' + k + ':\\s+(\\d+)', 'm'))[1]) * 1024;
  return {rss_bytes: get('Rss'), pss_bytes: get('Pss'), pid: process.pid,
          node_memory: process.memoryUsage()};
};
globalThis.bench_cpu_ns = cpu;
globalThis.process_snapshot = () => JSON.stringify(physical());
const start = process.hrtime.bigint();
const cpuStart = cpu();
const {loadPyodide} = await import(pathToFileURL(path.resolve(runtime, 'pyodide.mjs')));
const py = await loadPyodide({indexURL: path.resolve(runtime) + '/'});
const pythonSource = fs.readFileSync(path.join(path.dirname(fileURLToPath(import.meta.url)), 'final_python.py'), 'utf8');
py.globals.set('__name__', 'benchmark_l');
py.runPython(pythonSource);
const init = {kind: 'initialization', engine: 'pyodide-node', round: Number(arg || 0),
              runtime_init_wall_ns: Number(process.hrtime.bigint() - start),
              runtime_init_cpu_ns: cpu() - cpuStart,
              version: py.version, python: py.runPython('sys.version'), node: process.version,
              physical: physical(), includes: 'dynamic import, WASM compile/instantiate, stdlib initialization, benchmark Python imports; Node process startup separately recorded by controller'};
if (mode === 'init') { console.log(JSON.stringify(init)); process.exit(0); }
if (mode === 'memory') {
  py.globals.set('memory_code', arg);
  const result = await py.runPythonAsync("json.dumps(await memory_resident('pyodide-resident', memory_code))");
  for (const r of JSON.parse(result)) console.log(JSON.stringify(r));
  process.exit(0);
}
if (mode !== 'round') throw new Error('mode');
const ri = Number(arg);
const raw = path.join(OUT, 'pyodide.jsonl.gz');
if (ri === 0 && fs.existsSync(raw)) throw new Error('preserve raw');
if (ri > 0 && !fs.existsSync(raw)) throw new Error('missing previous round');
const commands = [];
let count = 0;
const collect = async cfg => {
  py.globals.set('config_json', JSON.stringify(cfg));
  const encoded = await py.runPythonAsync('json.dumps(await benchmark_embedded(json.loads(config_json)))');
  const rows = JSON.parse(encoded);
  const expected = cfg.reps * (cfg.kind === 'mixed' ? 40 : 1);
  if (rows.length !== expected || rows.some(r => !r.completed)) throw new Error('coverage');
  for (const r of rows) r.round = ri;
  fs.appendFileSync(raw, gzipSync(rows.map(r => JSON.stringify(r)).join('\n') + '\n'));
  count += rows.length; commands.push(cfg);
};
// Real smoke, then formal plan. Preserve warmup samples in formal raw.
py.globals.set('config_json', JSON.stringify({engine:'pyodide-resident',kind:'speed',case:'weighted',rows:100,reps:1}));
await py.runPythonAsync('json.dumps(await benchmark_embedded(json.loads(config_json)))');
let engines = ['pyodide-resident', 'pyodide-prepared'];
if (ri % 2) engines.reverse();
for (const [name, n] of [['scalar',0],['weighted',100],['stateful',100],['weighted',10000],['stateful',100000]]) {
  for (const engine of engines) await collect({engine,kind:'speed',case:name,rows:n,reps:21});
}
for (const mix of ['light', 'scan']) for (const delay_us of [0,1000]) {
  for (const engine of engines) await collect({engine,kind:'mixed',mix,delay_us,inflight:40,reps:5});
}
if (count !== 1810) throw new Error('round row count');
fs.appendFileSync(path.join(OUT, 'pyodide-initialization.jsonl'), JSON.stringify(init) + '\n');
fs.writeFileSync(path.join(OUT, `pyodide-identity-${ri}.json`), JSON.stringify({init,commands,raw_count:count,
  input:'same Python signed-64-array producer and tuple conversion, inside measured WASM execution; only small JSON request config crosses JS boundary',
  cpu:'Node process.cpuUsage includes Node + WASM; direct readings before/after request/window; no division of command CPU to fabricate per-request CPU'}, null, 2) + '\n');
console.log('Pyodide round', ri, 'complete', count);
