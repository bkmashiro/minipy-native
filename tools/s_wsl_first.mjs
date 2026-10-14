// One fresh Node/Pyodide instance, then its first common-source request.
// argv: RUNTIME CASE ROWS. Absolute hrtime uses Linux CLOCK_MONOTONIC.
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL, fileURLToPath} from 'node:url';
const [runtime, name, count] = process.argv.slice(2);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const initBegin = Number(process.hrtime.bigint());
const {loadPyodide} = await import(pathToFileURL(path.resolve(runtime, 'pyodide.mjs')));
const py = await loadPyodide({indexURL:path.resolve(runtime) + '/'});
py.globals.set('__name__', 'benchmark_s_first');
py.runPython(fs.readFileSync(path.join(root,'tools/final_python.py'),'utf8'));
py.globals.set('body',fs.readFileSync(path.join(root,'fixtures/common_' + name + '.py'),'utf8'));
py.globals.set('config_json',JSON.stringify({case:name,rows:Number(count)}));
py.runPython("cfg=json.loads(config_json)\ncfg['body']=body");
const ready = Number(process.hrtime.bigint());
py.runPython("code=prepare(cfg['case'], body)");
const prepared = Number(process.hrtime.bigint());
const encoded = await py.runPythonAsync('json.dumps(await execute(cfg, code))');
const executed = Number(process.hrtime.bigint());
const result = JSON.parse(encoded);
if (result.stdout !== py.runPython("oracle(cfg['case'], cfg['rows'])")) throw new Error('stdout');
console.log(JSON.stringify({kind:'first-request',engine:'pyodide-node',case:name,rows:Number(count),
  init_begin_ns:initBegin,ready_ns:ready,prepared_ns:prepared,executed_ns:executed,stdout:result.stdout,completed:true,
  prepare_boundary:'common body async wrapper compile including JS/Python bridge',
  execute_boundary:'generated input, conversion, tools, output, scope cleanup and small response bridge'}));
