"""L deterministic reduction. Run: python3 tools/final_summary.py.
Only current campaign raw; checks coverage before summarizing. No missing->zero.
"""
from collections import Counter, defaultdict
import gzip
import json
import math
from pathlib import Path
import statistics as st

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'bench/results/l-final'
PLAN = json.loads((OUT / 'plan.json').read_text())


def quantile(values, p):
    return sorted(values)[math.ceil(len(values) * p) - 1]


def shape_stat(rows):
    result = {'requests': len(rows), 'completed': sum(r['completed'] for r in rows)}
    for metric in ['wall_ns', 'cpu_ns', 'prepare_ns', 'execute_ns', 'conversion_ns', 'producer_setup_ns']:
        values = [r[metric] for r in rows if metric in r]
        result[metric.replace('_ns', '_mean_us')] = st.mean(values) / 1000 if values else None
    result['wall_median_us'] = st.median(r['wall_ns'] for r in rows) / 1000
    return result


def mixed_stat(rows):
    batches = defaultdict(list)
    for r in rows: batches[(r['round'], r['sample'])].append(r)
    assert all(len(rs) == 40 for rs in batches.values())
    firsts = [rs[0] for rs in batches.values()]
    for rs in batches.values():
        assert len({(r['batch_cpu_ns'], r['batch_wall_ns'], r['overloaded']) for r in rs}) == 1
        assert Counter((r['case'], r['rows']) for r in rs) == Counter({tuple(shape): w for shape, w in zip(PLAN['mixed']['shapes'], PLAN['mixed'][rs[0]['mix'] + '_weights'])})
    short = [r['latency_ns'] for r in rows if r['rows'] <= 100]
    return {'requests': len(rows), 'completed': sum(r['completed'] for r in rows),
            'windows': len(batches),
            'cpu_us_per_task': sum(r['batch_cpu_ns'] for r in firsts) / len(rows) / 1000,
            'mean_latency_us': st.mean(r['latency_ns'] for r in rows) / 1000,
            **{f'short_p{int(p * 100)}_us': quantile(short, p) / 1000 for p in [.50, .95, .99]},
            'drain_requests_per_s': len(rows) / (sum(r['batch_wall_ns'] for r in firsts) / 1e9),
            'finite_window_requests_per_s': len(rows) / (sum(max(100_000_000, r['batch_wall_ns']) for r in firsts) / 1e9),
            'overloaded_windows': sum(r['overloaded'] for r in firsts)}


def with_rounds(rows, function):
    result = function(rows)
    result['rounds'] = [function([r for r in rows if r['round'] == ri]) for ri in range(3)]
    result['round_ranges'] = {k: [min(r[k] for r in result['rounds']), max(r[k] for r in result['rounds'])]
                              for k, v in result.items() if isinstance(v, (float, int)) and k not in ['rounds', 'requests', 'completed', 'windows']}
    return result


def load_raw(path):
    with gzip.open(path, 'rt') as f: return [json.loads(line) for line in f]


def reduce_speed(rows, primary, common=False):
    shapes, mixed, all_overload = defaultdict(list), defaultdict(list), set()
    assert all(r['completed'] for r in rows)
    if not primary:
        family = 'cpython' if rows[0]['engine'].startswith('cpython') else 'pyodide'
        secondary_engines = [family+'-resident', family+'-prepared']
        if family == 'cpython' and not common: secondary_engines += ['cpython-spawn','cpython-fork']
        assert {r['engine'] for r in rows} == set(secondary_engines)
    for r in rows:
        if not primary:
            r['mode'] = 'warm' if r['engine'].endswith('prepared') else 'cold'
        if r['kind'] == 'speed':
            key = (r['engine'], r['case'], r['rows'], r['mode'])
            if r['sample'] >= (11 if common and primary else 21 if primary else 1): shapes[key].append(r)
        else:
            key = (r['engine'], r['mix'], r['mode'], r['delay_us'], r['inflight'])
            if r['overloaded']: all_overload.add((*key, r['round'], r['sample']))
            if r['sample'] >= (1 if common else 2 if primary else 1): mixed[key].append(r)
    engines = ['native','interpreter','monty'] if common else PLAN['primary_engines']
    shapes_plan = PLAN['secondary']['shapes'] if common else PLAN['shapes']
    expected = {(e, c, n, m) for e in engines for c, n in shapes_plan for m in PLAN['shape_modes']} if primary else {(e, c, n, 'warm' if e.endswith('prepared') else 'cold') for e in secondary_engines for c, n in PLAN['secondary']['shapes']}
    assert set(shapes) == expected, (set(shapes) ^ expected)
    result = {'shapes': [], 'mixed': [], 'all_raw_overloaded_windows': len(all_overload), 'raw_rows': len(rows)}
    for (e, c, n, m), rs in sorted(shapes.items()):
        assert len(rs) == (90 if common and primary else 240 if primary else 60)
        assert len({(r['round'], r['sample']) for r in rs}) == len(rs)
        result['shapes'].append(dict(engine=e, case=c, rows=n, mode=m, **with_rounds(rs, shape_stat)))
    expected_mixed = {(e, mix, mode, delay, 40) for e in PLAN['primary_engines'] for mix in ['light', 'scan'] for mode, delay in PLAN['mixed']['modes']} | {(e, 'scan', 'mixed-cold', 1000, cap) for e in PLAN['primary_engines'] for cap in [1,8,32]} if primary else None
    if primary:
        if common: expected_mixed = {(e,mix,mode,delay,40) for e in engines for mix in ['light','scan'] for mode,delay in PLAN['mixed']['modes']}
        assert set(mixed) == expected_mixed
    else:
        expected_mixed = {(e,mix,'warm' if e.endswith('prepared') else 'cold',delay,40) for e in secondary_engines for mix in ['light','scan'] for delay in [0,1000]}
        assert set(mixed) == expected_mixed
    for (e, mix, mode, delay, cap), rs in sorted(mixed.items()):
        assert len(rs) == (480 if common or not primary else 840)
        result['mixed'].append(dict(engine=e, mix=mix, mode=mode, delay_us=delay, inflight=cap,
                                   **with_rounds(rs, mixed_stat)))
    return result


def reduce_memory(rows, primary, validate=True):
    groups = defaultdict(list)
    for r in rows: groups[(r['engine'], r['code'], r['count'], r['phase'])].append(r)
    if primary and validate:
        expected = {(e, c, t, p) for e in PLAN['memory']['engines'] for c in PLAN['memory']['code'] for t in PLAN['memory']['tasks'] for p in PLAN['memory']['phases']}
        assert set(groups) == expected
    result = []
    for (engine, code, count, phase), rs in sorted(groups.items()):
        assert len(rs) == 3 and sorted(r['round'] for r in rs) == [0,1,2]
        record = {'engine': engine, 'code': code, 'tasks': count, 'phase': phase,
                  'rows_per_task': rs[0]['rows'], 'independent_processes': 3}
        for metric in ['rss_bytes', 'pss_bytes']:
            vals = [r['physical'][metric] for r in rs]
            assert all(v is not None for v in vals)
            record[metric] = {'mean': st.mean(vals), 'range': [min(vals), max(vals)], 'rounds': vals}
        traced = [r['allocation']['live_requested_bytes'] if primary else r['requested_traced_bytes'] for r in rs]
        record['requested_bytes'] = {'mean': st.mean(traced), 'range': [min(traced), max(traced)], 'rounds': traced} if all(v is not None for v in traced) else None
        record['allocation_scope'] = 'Rust requested layout bytes' if primary else 'Python tracemalloc, not whole runtime allocations'
        result.append(record)
    return result


def reduce_assembly(rows):
    groups = defaultdict(list)
    for r in rows:
        assert r['completed']
        if r['sample'] >= 21: groups[(r['engine'],r['mode'],r['case'],r['rows'],r['delay_us'])].append(r)
    expected = {(e,'kernel',c,n,0) for e in ['assembly','rust'] for c in ['weighted','stateful'] for n in [0,100,10000,100000]} | {(e,'full',c,n,d) for e in ['assembly','rust'] for c in ['weighted','stateful'] for n in [100,10000,100000] for d in [0,1000]}
    assert set(groups) == expected
    result=[]
    for (e,m,c,n,d),rs in sorted(groups.items()):
        assert len(rs)==240 and len({(r['round'],r['sample']) for r in rs})==240
        result.append(dict(engine=e,mode=m,case=c,rows=n,delay_us=d,**with_rounds(rs,shape_stat)))
    return {'shapes':result,'kernel_code_bytes':list({r['kernel_code_bytes'] for r in rows}),
            'numeric_state_bytes':16,'raw_count':len(rows)}


def main():
    identity = json.loads((OUT / 'identity.json').read_text())
    assert identity['completed_commands'] == identity['scheduled_commands'] == 414
    assert not identity['failed_commands']
    rows = load_raw(OUT / 'speed.jsonl.gz')
    mem = [json.loads(line) for line in (OUT / 'memory.jsonl').read_text().splitlines()]
    assert len(rows) == PLAN['speed_expected_rows'] == identity['speed_rows']
    assert len(mem) == PLAN['memory_expected_rows'] == identity['memory_rows']
    result = {'schema': 1, 'runtime_commit': PLAN['runtime_commit'],
              'environment': 'Linux ARM64 Alpine on M4, 2 vCPU / 2048 MiB',
              'primary': reduce_speed(rows, True), 'memory': reduce_memory(mem, True),
              'secondary': {}, 'secondary_memory': [], 'initialization': [], 'coverage': {}}
    for stem, expected_count in [('cpython', 10860), ('pyodide', 5430)]:
        path = OUT / (stem + '.jsonl.gz')
        if path.exists():
            sec = load_raw(path)
            assert len(sec) == expected_count
            result['secondary'][stem] = reduce_speed(sec, False)
            mp = OUT / (stem + '-memory.jsonl')
            if mp.exists(): result['secondary_memory'].extend(reduce_memory([json.loads(l) for l in mp.read_text().splitlines()], False))
            ip = OUT / (stem + '-initialization.jsonl')
            if ip.exists(): result['initialization'].extend(json.loads(l) for l in ip.read_text().splitlines())
    if (OUT/'addon-identity.json').exists():
        addon=json.loads((OUT/'addon-identity.json').read_text())
        if 'verified_counts' in addon:
            assert not addon['failures']
            result['common']={}
            for stem,expected,primary in [('primary',14490,True),('cpython',5430,False),('pyodide',5430,False)]:
                rs=load_raw(OUT/('common-'+stem+'.jsonl.gz')); assert len(rs)==expected
                result['common'][stem]=reduce_speed(rs,primary,common=True)
            rs=load_raw(OUT/'assembly.jsonl.gz'); assert len(rs)==12120
            result['assembly']=reduce_assembly(rs)
            ms=[json.loads(l) for l in (OUT/'assembly-memory.jsonl').read_text().splitlines()];assert len(ms)==36
            result['assembly_memory']=reduce_memory(ms,True,validate=False)
            v2_identity=OUT/'assembly-v2-identity.json'
            if v2_identity.exists():
                v2=json.loads(v2_identity.read_text())
                if 'verified_counts' in v2:
                    assert not v2['failures']
                    result['assembly_v1']=result['assembly']
                    result['assembly_v1_memory']=result['assembly_memory']
                    rs=load_raw(OUT/'assembly-v2.jsonl.gz');assert len(rs)==12120
                    result['assembly']=reduce_assembly(rs)
                    result['assembly']['revision']=2
                    ms=[json.loads(l) for l in (OUT/'assembly-v2-memory.jsonl').read_text().splitlines()];assert len(ms)==36
                    result['assembly_memory']=reduce_memory(ms,True,validate=False)
                    full_identity=OUT/'assembly-full-identity.json'
                    if full_identity.exists():
                        fi=json.loads(full_identity.read_text())
                        if 'verified_counts' in fi:
                            assert not fi['failures']
                            corrected=load_raw(OUT/'assembly-full.jsonl.gz');assert len(corrected)==7272
                            result['assembly_pre_host_fix']=result['assembly']
                            final_rows=[r for r in rs if r['mode']=='kernel']+corrected
                            assert len(final_rows)==12120
                            result['assembly']=reduce_assembly(final_rows)
                            result['assembly'].update(revision=2,kernel_cohort='assembly-v2.jsonl.gz',full_cohort='assembly-full.jsonl.gz')
                            ms=[json.loads(l) for l in (OUT/'assembly-full-memory.jsonl').read_text().splitlines()];assert len(ms)==36
                            result['assembly_memory']=reduce_memory(ms,True,validate=False)
    ip=OUT/'process-initialization.jsonl'
    if ip.exists(): result['initialization'].extend(json.loads(l) for l in ip.read_text().splitlines())
    result['coverage'] = {'primary_speed_cells': len(result['primary']['shapes']) + len(result['primary']['mixed']),
                          'primary_speed_expected_cells': 108,
                          'primary_memory_cells': len(result['memory']), 'primary_memory_expected_cells': 120,
                          'primary_requests': len(rows), 'primary_completed': len(rows),
                          'secondary_requests': {k: v['raw_rows'] for k,v in result['secondary'].items()},
                          'missing': [s for s in ['cpython', 'pyodide'] if s not in result['secondary']]}
    assert result['coverage']['primary_speed_cells'] == 108
    if 'common' in result:
        result['coverage']['common_speed_cells'] = sum(len(v['shapes'])+len(v['mixed']) for v in result['common'].values())
        result['coverage']['common_speed_expected_cells'] = 84
        assert result['coverage']['common_speed_cells']==84
        result['coverage']['common_requests'] = sum(v['raw_rows'] for v in result['common'].values())
        result['coverage']['assembly_speed_cells'] = len(result['assembly']['shapes'])
        assert result['coverage']['assembly_speed_cells']==40
        result['coverage']['assembly_requests'] = result['assembly']['raw_count']
        result['coverage']['assembly_memory_cells'] = len(result['assembly_memory'])
        assert result['coverage']['assembly_memory_cells']==12
    noise=OUT/'scalar-recheck.jsonl.gz'
    if noise.exists():
        rs=load_raw(noise);assert len(rs)==12012 and all(r['completed'] for r in rs)
        result['scalar_recheck']=[]
        for e in PLAN['primary_engines']:
            group=[r for r in rs if r['engine']==e and r['sample']>=201]
            assert len(group)==2400
            result['scalar_recheck'].append(dict(engine=e,**with_rounds(group,shape_stat)))
    (OUT / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result['coverage'], indent=2))
    for r in result['primary']['mixed']:
        if r['inflight'] == 40: print(r['engine'], r['mix'], r['mode'], r['delay_us'], round(r['cpu_us_per_task'],2), round(r['mean_latency_us'],2), round(r['short_p99_us'],2))


if __name__ == '__main__': main()
