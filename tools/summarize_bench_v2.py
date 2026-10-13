"""Aggregate raw v2 observations, preserve failures, no synthetic error bars."""
import collections
import json
import math
from pathlib import Path
import statistics
import sys


def median(values):
    values = [v for v in values if v is not None]
    return statistics.median(values) if values else None


def percentile(values, p):
    """Nearest-rank, observed samples. With tiny n, p99 often equals max."""
    return sorted(values)[max(0, math.ceil(len(values)*p)-1)] if values else None


def fit(points):
    """OLS observation, not a confidence interval; retain negative physical slopes."""
    if len(points) < 2 or len({x for x, _ in points}) < 2:
        return None
    mx, my = statistics.mean(x for x, _ in points), statistics.mean(y for _, y in points)
    slope = sum((x-mx)*(y-my) for x, y in points) / sum((x-mx)**2 for x, _ in points)
    return {'intercept': my-slope*mx, 'slope': slope, 'points': points}


def summarize(rows, runs):
    identity = next((r for r in rows if r.get('kind') == 'campaign_identity'), None)
    if identity:
        assert identity['scheduled_commands'] == len(runs), 'scheduled command count does not reconcile'
    failures = [r for r in rows if r.get('kind') in ['failure', 'command_failure']]
    latency = collections.defaultdict(list)
    concurrent = collections.defaultdict(list)
    requests = collections.defaultdict(list)
    memory = collections.defaultdict(list)
    for r in rows:
        kind = r.get('kind')
        arm = r.get('arm')
        if kind == 'latency':
            assert r['status'] == 'completed'
            assert sum(r['phases'].values()) <= r['total_wall_ns'], 'phase timers exceed whole request'
            latency[(arm, r['engine'], r['case'], r['rows'], r['lifecycle'], r['chunk'], r['delay_us'])].append(r)
        elif kind == 'concurrent':
            assert r['completed'] + r['failed'] == r['requests']
            concurrent[(arm, r['engine'], r['case'], r['rows'], r['concurrency'], r['chunk'], r['delay_us'], r['mixed'], r.get('lifecycle', 'prepared_shared_code_bounded_admission'), r.get('arrival_us') or 0)].append(r)
        elif kind == 'request':
            assert r['latency_ns'] == r['queue_ns'] + r['service_ns']
            requests[(r['command_index'], r['engine'])].append(r)
        elif kind == 'memory':
            memory[(arm, r['engine'], r['tasks'], r['rows'], r['bytes_per_task'], r['code'], r['cancel'], r['phase'])].append(r)
    speed = []
    for key, group in sorted(latency.items()):
        speed.append(dict(zip(['arm', 'engine', 'case', 'rows', 'lifecycle', 'chunk', 'delay_us'], key), observations=len(group), independent_runs=len({r['independent_run'] for r in group}), wall_ns=median([r['total_wall_ns'] for r in group]), cpu_ns=median([r['total_cpu_ns'] for r in group]), phases={p: median([r['phases'][p] for r in group]) for p in group[0]['phases']}))
    loads = []
    for key, group in sorted(concurrent.items()):
        tails, queues, services = [], [], []
        for r in group:
            req = requests[(r['command_index'], r['engine'])]
            assert len(req) == r['requests'], 'missing request rows'
            assert len({x['request'] for x in req}) == r['requests'], 'duplicate request rows'
            assert sum(x['status'] == 'completed' for x in req) == r['completed']
            short = [x for x in req if x['class'] == 'short']
            tails.extend(x['latency_ns'] for x in short)
            queues.extend(x['queue_ns'] for x in short)
            services.extend(x['service_ns'] for x in short)
        loads.append(dict(zip(['arm', 'engine', 'case', 'rows', 'concurrency', 'chunk', 'delay_us', 'mixed', 'lifecycle', 'arrival_us'], key), independent_runs=len(group), completed=sum(r['completed'] for r in group), failed=sum(r['failed'] for r in group), wall_ns=median([r['total_wall_ns'] for r in group]), cpu_ns=median([r['total_cpu_ns'] for r in group]), throughput_per_second=median([r['throughput_per_second'] for r in group]), short_request_count=len(tails), short_p50_ns=median(tails), short_p99_ns=percentile(tails, .99), short_queue_p99_ns=percentile(queues, .99), short_service_p99_ns=percentile(services, .99)))
    observations = []
    for key, group in memory.items():
        observations.append(dict(zip(['arm', 'engine', 'tasks', 'rows', 'bytes_per_task', 'code', 'cancel', 'phase'], key), independent_runs=len(group), live_requested_bytes=median([r['allocation']['live_requested_bytes'] for r in group]), peak_requested_bytes=median([r['allocation']['peak_requested_bytes'] for r in group]), rss_bytes=median([r['physical']['rss_bytes'] for r in group]), pss_bytes=median([r['physical']['pss_bytes'] for r in group]), peak_rss_bytes=median([r['physical']['peak_rss_bytes'] for r in group]), payload_logical_bytes=median([r['payload_live_logical_bytes'] for r in group])))
    # Use per-process engine_idle as baseline. Fits include diagnostic task handles.
    slopes = []
    bins = collections.defaultdict(list)
    for r in observations:
        if r['phase'] not in ['first_wait', 'second_wait'] or r['cancel']:
            continue
        idle = next((x for x in observations if all(x[k] == r[k] for k in ['arm', 'engine', 'tasks', 'rows', 'bytes_per_task', 'code', 'cancel']) and x['phase'] == 'engine_idle'), None)
        if idle is None:
            continue
        for metric in ['live_requested_bytes', 'rss_bytes', 'pss_bytes']:
            if r[metric] is None or idle[metric] is None:
                continue
            key = (r['arm'], r['engine'], r['rows'], r['bytes_per_task'], r['code'], r['phase'], metric)
            bins[key].append((r['tasks'], r[metric]-idle[metric]))
    for key, pts in bins.items():
        fitted = fit(pts)
        if fitted:
            slopes.append(dict(zip(['arm', 'engine', 'rows', 'bytes_per_task', 'code', 'phase', 'metric'], key), **fitted))
    # Fixed-task payload length controls: zero/10/4096 byte observations.
    byte_costs = []
    for arm, engine, tasks in {(r['arm'], r['engine'], r['tasks']) for r in observations if r['bytes_per_task'] is not None}:
        points = []
        for r in observations:
            if (r['arm'], r['engine'], r['tasks'], r['phase']) != (arm, engine, tasks, 'first_wait') or not tasks or r['bytes_per_task'] is None:
                continue
            idle = next(x for x in observations if all(x[k] == r[k] for k in ['arm', 'engine', 'tasks', 'bytes_per_task', 'code', 'cancel']) and x['phase'] == 'engine_idle')
            points.append((r['bytes_per_task'], (r['live_requested_bytes']-idle['live_requested_bytes'])/tasks))
        fitted = fit(points)
        if fitted:
            byte_costs.append({'arm': arm, 'engine': engine, 'tasks': tasks, 'metric': 'requested_layout_bytes_per_task', **fitted})
    break_even = []
    lookup = {(r['arm'], r['engine'], r['case'], r['rows'], r['lifecycle'], r['chunk'], r['delay_us']): r['wall_ns'] for r in speed}
    for arm, case, n, chunk, delay in {(r['arm'], r['case'], r['rows'], r['chunk'], r['delay_us']) for r in speed}:
        try:
            nc, mc, nw, mw = [lookup[(arm, engine, case, n, lifecycle, chunk, delay)] for engine, lifecycle in [('native', 'cold_source_to_result'), ('monty', 'cold_source_to_result'), ('native', 'prepared_request'), ('monty', 'prepared_request')]]
        except KeyError:
            continue
        crossing = max(1, math.ceil(1+(nc-mc)/(mw-nw))) if mw > nw else None
        break_even.append({'arm': arm, 'case': case, 'rows': n, 'chunk': chunk, 'delay_us': delay, 'modeled_reuse_requests': crossing, 'model': 'cold + (requests-1)*prepared median; no automatic cache, not observed reuse run'})
    statuses = collections.Counter(r['status'] for r in runs)
    assert sum(statuses.values()) == len(runs)
    return {'commands': {'scheduled': len(runs), **statuses}, 'failures': failures, 'latency': speed, 'break_even_model': break_even, 'concurrent': loads, 'memory': observations, 'task_slopes': slopes, 'byte_costs': byte_costs,
            'limitations': ['Speed uses default uninstrumented System allocator; memory is diagnostic and not a latency arm.', 'CPU getrusage is process-wide with microsecond resolution.', 'RSS/PSS are sampled current memory. Current Linux observed HWM is cumulative max(sampled smaps RSS, reported VmHWM), with raw kernel readings retained; it is not an exact continuously traced peak. macOS and historical probes use getrusage; see each raw physical.method. Requested allocator bytes exclude mmap/JIT/metadata.', 'Small physical payload differences may be below page/allocator resolution; negative sampled slopes are not clamped.', 'Native bytes is a touched Vec<u8> ownership probe; native str/bytes language unsupported.', 'Request percentiles within one concurrent run are not independent run error bars.']}


def main(directory):
    directory = Path(directory)
    rows = [json.loads(line) for line in (directory / 'v2-raw.jsonl').read_text().splitlines()]
    runs = [json.loads(line) for line in (directory / 'v2-runs.jsonl').read_text().splitlines()]
    result = summarize(rows, runs)
    (directory / 'v2-summary.json').write_text(json.dumps(result, indent=2) + '\n')
    text = ['# Benchmark v2 observations', '', 'Raw: v2-raw.jsonl. Commands/status: v2-runs.jsonl. Derived: v2-summary.json.', '', f"Commands: {result['commands']}", '', '## Latency (median microseconds)', '', '| Arm | Engine | Case | Rows | Lifecycle | Wall us | CPU us |', '|---|---|---|---:|---|---:|---:|']
    for r in result['latency']:
        text.append(f"| {r['arm']} | {r['engine']} | {r['case']} | {r['rows']} | {r['lifecycle']} | {r['wall_ns']/1000:.2f} | {r['cpu_ns']/1000:.2f} |")
    text += ['', '## Finite load observations', '', '| Arm | Engine | Lifecycle | Concurrency | Delay us/op | Requests/s | Short p99 us |', '|---|---|---|---:|---:|---:|---:|']
    for r in result['concurrent']:
        tail = '-' if r['short_p99_ns'] is None else f"{r['short_p99_ns']/1000:.2f}"
        text.append(f"| {r['arm']} | {r['engine']} | {r['lifecycle']} | {r['concurrency']} | {r['delay_us']} | {r['throughput_per_second']:.1f} | {tail} |")
    text += ['', '## Memory', '', 'See summary memory phase observations, task_slopes and byte_costs. Fits are diagnostic observations, not confidence intervals. Logical 10B is not RSS 10B.', '', '## Limits', ''] + ['- '+s for s in result['limitations']]
    (directory / 'report-v2.md').write_text('\n'.join(text)+'\n')
    print(json.dumps(result['commands']))


if __name__ == '__main__':
    main(sys.argv[1])
