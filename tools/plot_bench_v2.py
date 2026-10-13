"""Three bounded evidence figures from C Linux summary; no inferred error bars.
Run: uv run --with matplotlib python tools/plot_bench_v2.py
"""
import json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parents[1]
DIR = ROOT / 'bench/results/c-linux-v2'
S = json.loads((DIR / 'v2-summary.json').read_text())
OUT = ROOT / 'docs/benchmark-v2-figures'
OUT.mkdir(exist_ok=True)
plt.rcParams.update({'font.family': 'DejaVu Sans', 'font.size': 10,
                     'svg.fonttype': 'none', 'svg.hashsalt': 'minipy-v2',
                     'axes.spines.top': False, 'axes.spines.right': False})
COLORS = {'native': '#1664a4', 'monty': '#b75b16', 'monty-clear': '#55822b'}
LABELS = {'native': 'Native optimized', 'monty': 'Ordinary Monty', 'monty-clear': 'Monty, quotes = []'}
MIB = 1024 * 1024

def latency(engine, rows, life, arm='optimized', case='weighted_scan'):
    return next(r for r in S['latency'] if (r['arm'], r['engine'], r['rows'], r['lifecycle'], r['case']) == (arm, engine, rows, life, case))

def memory(engine, phase, tasks=8, rows=10000):
    return next(r for r in S['memory'] if (r['arm'], r['engine'], r['phase'], r['tasks'], r['rows'], r['code'], r['cancel']) == ('optimized', engine, phase, tasks, rows, 'shared', False))

def save(fig, name, caption):
    fig.text(.02, .015, caption, fontsize=9, va='bottom')
    fig.tight_layout(rect=(0, .12, 1, .95))
    fig.savefig(OUT / f'{name}.svg', metadata={'Date': None})
    fig.savefig(OUT / f'{name}.png', dpi=150)
    plt.close(fig)

fig, axes = plt.subplots(1, 2, figsize=(12, 4.8))
fig.suptitle('Compilation is recovered by enough rows or explicit reuse')
for engine in ['native', 'monty']:
    axes[0].plot(range(3), [latency(engine, n, 'cold_source_to_result')['wall_ns']/1000 for n in [0, 100, 10000]], marker='o', color=COLORS[engine], label=LABELS[engine])
axes[0].plot(range(3), [latency('native', n, 'cold_source_to_result', 'baseline-564381d')['wall_ns']/1000 for n in [0, 100, 10000]], marker='x', color='#767676', linestyle='--', label='Native baseline 564381d')
axes[0].set_xticks(range(3), ['0', '100', '10,000'])
axes[0].set_xlabel('Input rows (sampled sizes, categorical spacing)')
axes[0].set_ylabel('Source-to-result wall time (us, log scale)')
axes[0].set_yscale('log')
axes[0].set_title('Measured cold weighted_scan, ready provider')
axes[0].legend(fontsize=8)
counts = [1, 2, 3, 4, 8, 16, 32]
for engine in ['native', 'monty']:
    cold = latency(engine, 100, 'cold_source_to_result')['wall_ns']
    warm = latency(engine, 100, 'prepared_request')['wall_ns']
    axes[1].plot(counts, [(cold+(n-1)*warm)/n/1000 for n in counts], marker='o', linestyle='--', color=COLORS[engine], label=LABELS[engine])
axes[1].axvline(3, linestyle=':', color='#888888')
axes[1].set_xscale('log', base=2)
axes[1].set_xticks([1, 2, 4, 8, 16, 32], ['1', '2', '4', '8', '16', '32'])
axes[1].set_xlabel('Requests per explicit code reuse (modeled)')
axes[1].set_ylabel('Modeled average wall time (us/request)')
axes[1].set_ylim(bottom=0)
axes[1].set_title('100 rows: modeled crossover at 3 requests')
axes[1].legend(fontsize=8)
save(fig, 'cold-and-reuse', 'Linux ARM64 VM; pooled median of 21 requests from 3 process runs per measured cell. No confidence intervals.\nRight: [cold + (R-1)*prepared] / R, derived from separate timing arms; no observed reuse run or automatic cache.')

fig, axes = plt.subplots(1, 3, figsize=(15, 5))
fig.suptitle('Waiting-state allocation and process RSS are different costs')
for engine in ['native', 'monty']:
    tasks = [0, 1, 8, 32]
    ys = [(memory(engine, 'first_wait', n, 0)['live_requested_bytes'] - memory(engine, 'engine_idle', n, 0)['live_requested_bytes'])/1024 for n in tasks]
    axes[0].plot(tasks, ys, marker='o', color=COLORS[engine], label=LABELS[engine])
axes[0].set_xlabel('Live tasks (zero Quote rows, shared code)')
axes[0].set_ylabel('First-wait requested allocation above idle (KiB)')
axes[0].set_ylim(bottom=0)
axes[0].set_title('Full task/provider diagnostic path')
axes[0].legend(fontsize=8)
phases = ['engine_idle', 'first_wait', 'second_wait', 'completed']
labels = ['Idle', 'Load wait', 'Record wait', 'Done']
for engine in ['native', 'monty', 'monty-clear']:
    for ax, metric in [(axes[1], 'live_requested_bytes'), (axes[2], 'rss_bytes')]:
        ax.plot(range(4), [memory(engine, p)[metric]/MIB for p in phases], marker='o', color=COLORS[engine], label=LABELS[engine])
for ax in axes[1:]:
    ax.set_xticks(range(4), labels, rotation=15)
    ax.set_xlabel('Lifecycle snapshot (not elapsed time)')
    ax.set_ylim(bottom=0)
    ax.legend(fontsize=8)
axes[1].set_ylabel('Live requested Rust allocation (MiB)')
axes[1].set_title('8 tasks x 10,000 rows, shared code')
axes[2].set_ylabel('Whole-process sampled RSS (MiB)')
axes[2].set_title('Same tasks, separate physical metric')
save(fig, 'waiting-memory', 'Linux ARM64; medians of 3 independent diagnostic processes. Allocator counters exclude metadata and JIT mmap.\nMonty-clear rebinds quotes = []; one loop row remains. VM capacity survives logical release. No trim calls or confidence intervals.')

fig, axes = plt.subplots(1, 2, figsize=(12, 5))
fig.suptitle('Finite cold arrivals: same offered schedule, different backlog and CPU')
for engine in ['native', 'monty']:
    rows = [next(r for r in S['concurrent'] if r['arm']=='optimized' and r['engine']==engine and r['lifecycle']=='cold_fixed_arrival_bounded_admission' and r['delay_us']==delay) for delay in [0, 1000]]
    offset = -.16 if engine=='native' else .16
    axes[0].bar([i+offset for i in range(2)], [r['short_p99_ns']/1e6 for r in rows], width=.3, color=COLORS[engine], label=LABELS[engine])
    axes[1].bar([i+offset for i in range(2)], [r['cpu_ns']/1e6 for r in rows], width=.3, color=COLORS[engine], label=LABELS[engine])
for ax in axes:
    ax.set_xticks(range(2), ['Ready', '1 ms sleep per operation'])
    ax.set_xlabel('Provider mode (two operations per request)')
    ax.set_ylim(bottom=0)
    ax.legend(fontsize=8)
axes[0].set_ylabel('Pooled short-request p99 arrival-to-result (ms)')
axes[0].set_title('144 short observations, 3 process runs')
axes[1].set_ylabel('Median whole-burst process CPU (ms)')
axes[1].set_title('Compilation + input + execution + scheduling')
save(fig, 'cold-arrival', '64 mixed requests/run; one arrival/ms; max 8 admitted tasks; 25% 10,000-row heavy, 75% 100-row short.\nFresh compilation on a single reactor. Queue includes timer/reactor lateness. Finite load, not production capacity; no confidence intervals.')
print(OUT)
