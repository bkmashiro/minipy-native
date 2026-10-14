// One serial, resident Pyodide curve cell using unchanged common body.
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL, fileURLToPath} from 'node:url';
const [runtime,encoded] = process.argv.slice(2);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const {loadPyodide} = await import(pathToFileURL(path.resolve(runtime,'pyodide.mjs')));
const py = await loadPyodide({indexURL:path.resolve(runtime)+'/'});
globalThis.bench_cpu_ns = () => {const c=process.cpuUsage();return (c.user+c.system)*1000;};
py.globals.set('__name__','benchmark_s_curve');
py.runPython(fs.readFileSync(path.join(root,'tools/final_python.py'),'utf8'));
const cfg=JSON.parse(encoded);
cfg.bodies=Object.fromEntries(['scalar','weighted','stateful'].map(c=>[c,fs.readFileSync(path.join(root,'fixtures/common_'+c+'.py'),'utf8')]));
py.globals.set('config_json',JSON.stringify(cfg));
const result=JSON.parse(await py.runPythonAsync('json.dumps(await benchmark_embedded(json.loads(config_json)))'));
if(result.length!==cfg.reps || result.some(r=>!r.completed))throw new Error('coverage');
for(const r of result)console.log(JSON.stringify(r));
