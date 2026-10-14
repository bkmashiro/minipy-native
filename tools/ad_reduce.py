"""AD strict reduction, compatible with existing S/W import row fields.
python3 ad_reduce.py BASE [SUMMARY_HELPER]
No execution. Refuses different identity, missing samples, mixed cohorts or rounds.
"""
from collections import Counter, defaultdict
import gzip
import hashlib
import json
from pathlib import Path
import runpy
import statistics as st
import sys
from typing import Any
import ad_campaign as campaign


def stats(xs) -> dict[str, Any]:
    return dict(mean=st.mean(xs), range=[min(xs), max(xs)], rounds=xs)


def reduce(base, helper, replay=None):
    out = replay if replay is not None else base/'ad-corrected'
    helpers = runpy.run_path(str(helper))
    plan = json.loads(((out/'plan.json') if replay is not None else (base/'plan.json')).read_text())
    work = json.loads((out/'commands.json').read_text())
    status = json.loads((out/'status.json').read_text())
    identity = json.loads((out/'identity.json').read_text())
    assert work == campaign.schedule(base)
    assert len(status) == 300 and len({s['key'] for s in status}) == 300
    assert {s['key'] for s in status} == {i['key'] for i in work}
    assert all(s['state'] == 'completed' for s in status)
    if replay is None:
        assert all(hashlib.sha256(Path(p).read_bytes()).hexdigest() == h['sha256'] for p, h in identity['files'].items())
    allrows, raw_identity = [], {}
    bundled = defaultdict(list)
    if replay is not None:
        with gzip.open(out/'raw.jsonl.gz', 'rt') as f:
            for line in f:
                r=json.loads(line); bundled[r['command_key']].append(r)
        assert set(bundled) == {i['key'] for i in work}
    for item in work:
        if replay is None:
            path = out/(item['key']+'.jsonl.gz')
            raw_identity[path.name] = dict(sha256=hashlib.sha256(path.read_bytes()).hexdigest(), bytes=path.stat().st_size)
            with gzip.open(path, 'rt') as f: rows = [json.loads(line) for line in f]
        else:
            rows = bundled[item['key']]
        campaign.validate(rows, item)
        allrows.extend(rows)
    assert len(allrows) == 12069
    assert Counter(r['kind'] for r in allrows) == Counter(speed=1980, mixed=9600, hold=153, **{'first-request':36, 'initialization':300})
    sg, mg, hg, fg = (defaultdict(list) for _ in range(4))
    for r in allrows:
        kind = r['kind']
        if kind in ['speed','mixed'] and r['sample'] < r['discard']: continue
        if kind == 'speed': sg[r['engine'],r['case'],r['rows']].append(r)
        elif kind == 'mixed': mg[r['engine'],r['mix'],r['delay_us']].append(r)
        elif kind == 'hold': hg[r['engine'],r['count'],r['phase']].append(r)
        elif kind == 'first-request': fg[r['engine'],r['case'],r['rows']].append(r)
    speed, mixed, memory, first = [], [], [], []
    tag = dict(corpus='ad-json-object', cohort=campaign.COHORT)
    for (engine,case,n), rows in sorted(sg.items()):
        assert Counter(r['round'] for r in rows) == Counter({0:10,1:10,2:10})
        speed.append(dict(tag, engine=engine, case=case, rows=n, mode='cold',
                          **helpers['by_round'](rows, helpers['shapes'])))
    for (engine,mix,delay), rows in sorted(mg.items()):
        assert Counter(r['round'] for r in rows) == Counter({0:160,1:160,2:160})
        mixed.append(dict(tag, engine=engine, mix=mix, delay_us=delay, mode='mixed-cold', inflight=40,
                          **helpers['by_round'](rows, lambda rs: helpers['mixed'](rs, plan))))
    for (engine,tasks,phase), rows in sorted(hg.items()):
        rows = sorted(rows, key=lambda r:r['round'])
        assert [r['round'] for r in rows] == [0,1,2]
        # Match the actual PID set: fork's task holds use its living empty children,
        # reclaim uses the original controller-only tree after all children exit.
        baseline_phase = 'empty_tree' if engine == 'cpython-fork-json' and phase in ['load_wait','record_wait','empty_tree'] else 'startup'
        baselines = sorted(hg[engine,tasks,baseline_phase], key=lambda r:r['round'])
        assert all(set(r['physical']['tree_pids']) == set(b['physical']['tree_pids']) for r,b in zip(rows,baselines))
        record: dict[str, Any] = dict(tag, engine=engine, code='runner-shared', tasks=tasks, phase=phase,
                      rows=10000, independent_processes=3, requested_bytes=None,
                      baseline_phase=baseline_phase, baseline_pid_set_matched=True)
        for name in ['rss_bytes','pss_bytes']:
            xs = [r['physical'][name] for r in rows]
            bs = [r['physical'][name] for r in baselines]
            record[name] = stats(xs)
            record['baseline_'+name] = stats(bs)
            record['incremental_'+name] = stats([x-b for x,b in zip(xs,bs)])
        memory.append(record)
    for (engine,case,n), rows in sorted(fg.items()):
        rows = sorted(rows, key=lambda r:r['round'])
        assert [r['round'] for r in rows] == [0,1,2]
        assert all(sum(r[p] for p in campaign.PHASES) == r['wall_ns'] for r in rows)
        metrics: dict[str, Any] = {k:dict(mean_ms=st.mean(r[k] for r in rows)/1e6,
                         median_ms=st.median(r[k] for r in rows)/1e6,
                         range_ms=[min(r[k] for r in rows)/1e6,max(r[k] for r in rows)/1e6],
                         rounds_ms=[r[k]/1e6 for r in rows]) for k in campaign.PHASES+['wall_ns','cpu_ns']}
        assert abs(sum(metrics[k]['mean_ms'] for k in campaign.PHASES)-metrics['wall_ns']['mean_ms']) < 1e-8
        first.append(dict(tag, engine=engine, case=case, rows=n, independent_processes=3,
                          metrics=metrics, prepare_boundary=rows[0]['prepare_boundary'],
                          execute_boundary=rows[0]['execute_boundary']))
    assert tuple(map(len,[speed,mixed,memory,first])) == (60,16,51,12)
    summary = dict(schema=1, cohort=campaign.COHORT, speed=speed, mixed=mixed, memory=memory, first=first,
        coverage=dict(scheduled_commands=300,completed_commands=300,failed_commands=0,unsupported_commands=0,
            scheduled_rows=11769,completed_rows=11769,initialization_rows=300,speed_cells=60,mixed_cells=16,memory_cells=51,first_cells=12),
        tool_response_limit=8<<20,
        largest_response_bytes=len(json.dumps({'value':[dict(quantity=i%9-2,unit_price_cents=i*37%1000) for i in range(100000)]},separators=(',',':')).encode()),
        native_transport='real localhost TCP JSON per control and Tool call; newline framing, same Go map producer and compact value envelope; no precomputed payloads',
        native_cpu='Go RUSAGE_SELF + kernel native parent process CPUCLOCK_SCHED + cumulative RUSAGE_CHILDREN after per-request waitpid, monotonically collected from run responses; no CPU sampling RPC',
        lifecycle=dict(cow='prepared DataImage, new instance/close per request; Wasm compile/image capture outside request',
            fresh='new CPython Guest instance/close per request; Wasm compile outside request',
            fork='single-threaded import-prewarmed gc.freeze parent; new fork, compile/exec, exit, IPC and waitpid per request; no pool/code cache',
            resident='async interpreter process persists; async wrapper adapted/preloaded outside request; compile each request; no child isolation'),
        memory_baseline='Use each row baseline_phase/baseline_pss_bytes/incremental_pss_bytes. Fork load/record subtract empty_tree with identical child PIDs; reclaimed subtract controller startup after child reap. Do not substitute controller startup for living fork children.')
    if replay is not None:
        assert summary == json.loads((out/'import-summary.json').read_text()), 'summary differs from bundled raw'
        print('REPLAY PASS: 300 commands, 12069 raw rows, all aggregate values exact; remote artifacts not revalidated')
        return summary
    with gzip.open(out/'raw.jsonl.gz', 'wt') as f:
        for row in allrows: f.write(json.dumps(row)+'\n')
    (out/'import-summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    (out/'raw-identity.json').write_text(json.dumps(raw_identity, indent=2)+'\n')
    replacement = dict(schema=1, cohort=campaign.COHORT, import_file='import-summary.json',
        remove_current_engines=['pysolate-cow','pysolate-fresh'], remove_from=['curve','mixed','memory','first','initialization'],
        removed_cohorts=['w-expanded-tool-response','old-pysolate-adapted'],
        add_engines=['pysolate-cow','pysolate-fresh','cpython-fork-json','cpython-resident-json'],
        primary_baseline='cpython-fork-json', optional_resident='cpython-resident-json',
        request_mapping='speed -> curve; scalar and seven sizes for both scan cases. Replace whole Pysolate arms, never fill gaps from S/W.',
        memory_mapping='memory raw startup remains controller-only for fork. Import precomputed incremental_pss_bytes and its row-specific matched baseline. Include empty_tree as diagnostic, not additional task stage.',
        unchanged=dict(source='existing S + extension summaries',engines='MiniPy native/interpreter, Monty, Rust/assembly, PyPy, Pyodide and original namedtuple CPython lifecycles',rule='Preserve original cohort identity. Original cpython-fork/resident are separate common/namedtuple context, not this matched JSON baseline.'),
        group='ad-json-object: same Go-generated Quote fields, JSON dict + SimpleNamespace; paired subset, not silently the typed/namedtuple common group',
        first_mapping='First-request now 12 cells, each 3 independent fresh processes. All five mean phases sum to full launch-through-reap wall. source_prepare is host adaptation, Python compile remains inside execute. Oracle is controller-side after wall/CPU end.')
    (out/'replacement.json').write_text(json.dumps(replacement, indent=2)+'\n')
    (out/'verification.json').write_text(json.dumps(dict(coverage=summary['coverage'],
        identity_files_verified=len(identity['files']),raw_files_verified=len(raw_identity),rounds_per_cell=3,
        first_exact_phase_sum=True,matched_memory_pid_sets=True,raw_directory=str(out),
        lock_contract=identity['locks'],source_commit=identity['source_commit']), indent=2)+'\n')
    print(json.dumps(summary['coverage'], indent=2))
    return summary


if __name__ == '__main__':
    if sys.argv[1] == '--replay':
        out=Path(sys.argv[2]).resolve()
        commands=json.loads((out/'commands.json').read_text())
        base=Path(commands[0]['argv'][3]).parents[1]
        reduce(base, Path(__file__).with_name('s_wsl_summary.py'), replay=out)
    else:
        base = Path(sys.argv[1]).resolve()
        reduce(base, Path(sys.argv[2]) if len(sys.argv)>2 else base/'source/tools/s_wsl_summary.py')
