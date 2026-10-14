"""L figures from current summary only. uv run --with matplotlib python tools/final_plot.py"""
import json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
S = json.loads((ROOT/'bench/results/l-final/summary.json').read_text())
OUT = ROOT/'docs/figures'
OUT.mkdir(exist_ok=True)
plt.rcParams.update({'font.family':'DejaVu Sans','font.size':10,'axes.titlesize':12,
                     'axes.labelsize':10,'svg.fonttype':'none','svg.hashsalt':'minipy-l',
                     'axes.spines.top':False,'axes.spines.right':False,'figure.facecolor':'white'})
COLORS = {'native':'#1768AC','interpreter':'#079078','monty':'#D27818',
          'reference':'#677787','monty-clear':'#B99C3F','cpython-resident':'#8463BA',
          'cpython-prepared':'#B48AD4','cpython-spawn':'#BD496B','cpython-fork':'#A17A59',
          'pyodide-resident':'#3B899C','pyodide-prepared':'#72ACB8'}
LABEL = {'native':'MiniPy codegen','interpreter':'MiniPy typed IR','monty':'Monty',
         'reference':'Fixed Rust reference','monty-clear':'Monty list-cleared',
         'cpython-resident':'CPython resident / fresh source',
         'cpython-prepared':'CPython resident / prepared source',
         'cpython-spawn':'CPython spawn / fresh source',
         'cpython-fork':'CPython prewarm + fork / fresh source',
         'pyodide-resident':'Pyodide resident / fresh source',
         'pyodide-prepared':'Pyodide resident / prepared source'}


def find_shape(engine, case, rows, mode):
    pool = S['primary']['shapes'] + [r for group in S['secondary'].values() for r in group['shapes']]
    return next(r for r in pool if (r['engine'],r['case'],r['rows'],r['mode'])==(engine,case,rows,mode))


def find_mix(engine, mix, delay):
    pool = S['primary']['mixed'] + [r for group in S['secondary'].values() for r in group['mixed']]
    return next(r for r in pool if r['engine']==engine and r['mix']==mix and r['delay_us']==delay and r['inflight']==40 and r['mode'] in ['mixed-cold','cold','warm'])


def range_mark(ax, x, row, metric, engine, marker='o', label=None):
    value = row[metric]
    lo, hi = row['round_ranges'][metric]
    # A pooled statistic can lie outside the range of separate round quantiles.
    ax.vlines(x, lo, hi, color=COLORS[engine], lw=1.5, alpha=.7)
    ax.plot(x, value, marker=marker, color=COLORS[engine], markersize=5,
            linestyle='none', label=label)


def style(ax, ylabel, xlabel, log=False):
    ax.set_ylabel(ylabel); ax.set_xlabel(xlabel)
    if log: ax.set_yscale('log')
    ax.grid(axis='y',alpha=.16)
    ax.set_axisbelow(True)


def save(fig, name, note):
    fig.text(.025,.017,note,fontsize=9,color='#48515C',va='bottom')
    fig.tight_layout(rect=(0,.07,1,.91),h_pad=2.5,w_pad=2.5)
    fig.savefig(OUT/(name+'.svg'),metadata={'Date':None})
    fig.savefig(OUT/(name+'.png'),dpi=170)
    plt.close(fig)


fig, axs = plt.subplots(2,3,figsize=(16,10))
fig.suptitle('Frozen MiniPy: preparation, execution, queueing and waiting cost',fontsize=18,x=.025,ha='left',y=.98)
fig.text(.025,.944,'8a293ec · Linux ARM64 / Alpine · Apple M4 VM: 2 vCPU, 2 GiB · synthetic fixtures · 3 rotated rounds',fontsize=11,color='#556170')
for ax,case,mode,title in [(axs[0,0],'weighted','cold','A  Weighted: source-cold end to end'),
                          (axs[0,1],'weighted','warm','B  Weighted: explicit prepared reuse'),
                          (axs[0,2],'stateful','cold','C  Stateful: source-cold end to end')]:
    engines = ['native','interpreter','monty'] + (['reference'] if mode=='warm' else [])
    for engine in engines:
        ns=[100,10000,100000]
        rs=[find_shape(engine,case,n,mode) for n in ns]
        ax.plot(ns,[r['wall_mean_us'] for r in rs],color=COLORS[engine],lw=1.8,
                linestyle='--' if engine=='reference' else '-',label=LABEL[engine])
        for n,r in zip(ns,rs): range_mark(ax,n,r,'wall_mean_us',engine)
    ax.set_xscale('log'); ax.set_title(title,loc='left')
    ax.set_xticks([100,10000,100000],['100','10,000','100,000'])
    style(ax,'Mean request wall time (µs, log)','Input rows (log)',True)
    if mode=='warm': ax.legend(fontsize=9,frameon=False,loc='upper left')
axs[0,0].legend(fontsize=9,frameon=False,loc='upper left')
# CPU covers all requests, rather than averaging per-shape medians.
ax=axs[1,0]; ax.set_title('D  Cold mix: whole-process CPU',loc='left')
for ei,engine in enumerate(['native','interpreter','monty']):
    for xi,mix in enumerate(['light','scan']):
        range_mark(ax,xi+(ei-1)*.16,find_mix(engine,mix,0),'cpu_us_per_task',engine,
                   label=LABEL[engine] if xi==0 else None)
ax.set_xticks([0,1],['Light mix','Scan mix']); style(ax,'CPU per completed request (µs)','Artificial mix')
ax.set_ylim(bottom=0); ax.legend(fontsize=9,frameon=False)
ax=axs[1,1]; ax.set_title('E  Cold short-task tail, planned arrival',loc='left')
conditions=[('light',0),('scan',0),('light',1000),('scan',1000)]
for ei,engine in enumerate(['native','interpreter','monty']):
    for xi,(mix,delay) in enumerate(conditions):
        range_mark(ax,xi+(ei-1)*.13,find_mix(engine,mix,delay),'short_p99_us',engine,
                   label=LABEL[engine] if xi==0 else None)
ax.set_xticks(range(4),['Light\nready','Scan\nready','Light\n2×1 ms','Scan\n2×1 ms'])
style(ax,'Pooled short p99 (µs, log)','Artificial mix / actual Tool waits',True)
ax=axs[1,2]; ax.set_title('F  Record wait: 8 tasks, unique preparation',loc='left')
engines=['native','interpreter','reference','monty','monty-clear']
for xi,e in enumerate(engines):
    r=next(r for r in S['memory'] if (r['engine'],r['tasks'],r['code'],r['phase'])==(e,8,'unique','record_wait'))
    ax.vlines(xi,r['pss_bytes']['range'][0]/2**20,r['pss_bytes']['range'][1]/2**20,color=COLORS[e])
    ax.plot(xi,r['pss_bytes']['mean']/2**20,'o',color=COLORS[e])
ax.set_xticks(range(5),['Codegen','Typed IR','Fixed Rust\nreference','Monty','Monty\nlist-cleared'])
style(ax,'Whole-process PSS (MiB, log)','Engine · weighted 10,000 rows/task',True)
save(fig,'benchmark-overview','Markers: pooled/overall statistic; thin lines: observed range of 3 round statistics (not a confidence interval).\nCold panels exclude the fixed Rust reference: it has no source preparation. Finite 40-request windows are not a sustained service SLO.')

if not {'cpython','pyodide'} <= set(S['secondary']):
    raise SystemExit('Secondary figure requires actual CPython and Pyodide results')
fig,axs=plt.subplots(2,2,figsize=(15,11))
fig.suptitle('Ordinary Python lifecycles: end-to-end time and participating-process memory',fontsize=17,x=.025,ha='left',y=.98)
fig.text(.025,.944,'Same Linux ARM64 VM · CPython 3.14.7 · Pyodide 314.0.7 / Node 24.18.1 · successful bounded synthetic corpus',fontsize=11,color='#556170')
engines=['cpython-spawn','cpython-fork','cpython-resident','cpython-prepared','pyodide-resident','pyodide-prepared']
short_labels=['CPython\nspawn','CPython\nprewarm+fork','CPython\nresident cold','CPython\nprepared','Pyodide\nresident cold','Pyodide\nprepared']
for ax,case,n,title in [(axs[0,0],'scalar',0,'A  Small scalar: actual request lifecycle'),
                         (axs[0,1],'weighted',10000,'B  Weighted 10,000 rows: actual request lifecycle')]:
    for xi,e in enumerate(engines):
        mode='warm' if e.endswith('prepared') else 'cold'
        r=find_shape(e,case,n,mode)
        range_mark(ax,xi,r,'wall_mean_us',e)
    ax.set_xticks(range(6),short_labels,fontsize=9)
    ax.set_title(title,loc='left'); style(ax,'Mean request wall time (µs, log)','Execution lifecycle',True)
ax=axs[1,0]; ax.set_title('C  Scan mix: mean planned-arrival latency',loc='left')
for xi,e in enumerate(engines):
    for delay,offset,marker in [(0,-.12,'o'),(1000,.12,'s')]:
        r=find_mix(e,'scan',delay)
        range_mark(ax,xi+offset,r,'mean_latency_us',e,marker)
ax.set_xticks(range(6),short_labels,fontsize=9)
style(ax,'Mean request latency (µs, log)','Execution lifecycle · 40 finite-window arrivals',True)
from matplotlib.lines import Line2D
ax.legend(handles=[Line2D([],[],color='#536170',marker='o',ls='none',label='Ready'),
                   Line2D([],[],color='#536170',marker='s',ls='none',label='Two actual 1 ms waits')],frameon=False,fontsize=9)
ax=axs[1,1]; ax.set_title('D  Record wait: all participating processes',loc='left')
memory_engines=['native','interpreter','monty','cpython-resident','cpython-spawn','cpython-fork','pyodide-resident']
for xi,e in enumerate(memory_engines):
    pool=S['memory'] if e in ['native','interpreter','monty'] else S['secondary_memory']
    r=next(r for r in pool if (r['engine'],r['tasks'],r['code'],r['phase'])==(e,8,'unique','record_wait'))
    ax.vlines(xi,r['pss_bytes']['range'][0]/2**20,r['pss_bytes']['range'][1]/2**20,color=COLORS[e])
    ax.plot(xi,r['pss_bytes']['mean']/2**20,'o',color=COLORS[e])
ax.set_xticks(range(7),['MiniPy\ncodegen','MiniPy\ntyped IR','Monty','CPython\nresident','CPython\nspawn','CPython\nfork','Pyodide\nresident'],fontsize=9)
style(ax,'Process / process-tree PSS (MiB, log)','8 waiting tasks · weighted 10,000 rows/task',True)
save(fig,'benchmark-python-lifecycles','Resident request values exclude interpreter initialization; fresh-process initialization is reported separately in the evidence table.\nPython array-to-tuple construction is timed. Ordinary Python scans/integers/authority differ from MiniPy; these are not equivalent sandbox rankings.\nMemory sums participating-process PSS (including fork parent); requested-allocation counters are not compared across runtimes.')
print(OUT/'benchmark-overview.svg'); print(OUT/'benchmark-python-lifecycles.svg')
