"""S small SVG: primary common-body costs and suspended whole-process PSS.
Run: python3 tools/s_wsl_plot.py SUMMARY OUTPUT.svg
Marks are three-round means with min/max round ranges, not confidence intervals.
"""
import html
import json
import math
from pathlib import Path
import sys

ENGINES=[('native','#2563eb'),('interpreter','#d97706'),('monty','#dc2626'),('reference','#64748b')]
SHAPES=[('scalar',0),('weighted',100),('stateful',100),('weighted',10000),('stateful',100000)]
LABELS=['scalar','weighted 100','stateful 100','weighted 10k','stateful 100k']


def plot(summary,path):
    parts=['<svg xmlns="http://www.w3.org/2000/svg" width="1500" height="530" viewBox="0 0 1500 530">',
           '<rect width="1500" height="530" fill="white"/>',
           '<style>text{font:14px sans-serif;fill:#172033}.tick{font-size:12px}.title{font-size:18px;font-weight:600}</style>']
    def text(x,y,value,cls='',anchor='start'):
        parts.append(f'<text x="{x}" y="{y}" class="{cls}" text-anchor="{anchor}">{html.escape(value)}</text>')
    def line(x1,y1,x2,y2,color='#d9dfe8',width=1):
        parts.append(f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="{width}"/>')
    def mark(x,y,low,high,color):
        line(x,low,x,high,color,2);line(x-3,low,x+3,low,color,2);line(x-3,high,x+3,high,color,2)
        parts.append(f'<circle cx="{x}" cy="{y}" r="4" fill="{color}"/>')
    text(30,28,'S · frozen compact interpreter · Linux x86_64 WSL2','title')
    text(30,52,'Whole request wall includes provider construction/conversion and teardown; resident process startup excluded.')
    for i,(engine,color) in enumerate(ENGINES):
        parts.append(f'<circle cx="{40+i*230}" cy="80" r="4" fill="{color}"/>')
        text(52+i*230,85,engine+(' (handwritten lower bound)' if engine=='reference' else ''))
    for panel,mode in enumerate(['cold','warm']):
        left=65+panel*495;top=145;bottom=415;width=405
        records=[r for r in summary['speed'] if r['engine'] in dict(ENGINES) and r['mode']==mode]
        vals=[v for r in records for v in r['round_ranges']['wall_mean_us']]
        low=math.floor(math.log10(min(vals)));high=math.ceil(math.log10(max(vals)))
        def y(v):return bottom-(math.log10(v)-low)/(high-low)*(bottom-top)
        text(left,120,('Prepare + run' if mode=='cold' else 'Prepared run')+' · wall µs (log axis)','title')
        for exp in range(low,high+1):
            yy=y(10**exp);line(left,yy,left+width,yy);text(left-8,yy+4,f'{10**exp:g}','tick','end')
        for s,(case,n) in enumerate(SHAPES):
            xx=left+35+s*82
            text(xx,bottom+22,LABELS[s],'tick','middle')
            for ei,(engine,color) in enumerate(ENGINES):
                r=next(r for r in records if (r['engine'],r['case'],r['rows'])==(engine,case,n))
                a,b=r['round_ranges']['wall_mean_us'];mark(xx+(ei-1.5)*10,y(r['wall_mean_us']),y(a),y(b),color)
    left=1070;right=1475;top=145;bottom=415
    records=[r for r in summary['memory'] if r['engine'] in dict(ENGINES) and r['tasks']==32 and r['code']=='shared']
    phases=['startup','load_wait','record_wait','reclaimed']
    maximum=math.ceil(max(v for r in records for v in r['pss_bytes']['range'])/1048576/10)*10
    def memory_y(v):return bottom-v/maximum*(bottom-top)
    text(left,120,'32 tasks · shared code · PSS MiB','title')
    for tick in range(0,int(maximum)+1,max(1,int(maximum)//5)):
        yy=memory_y(tick);line(left,yy,right,yy);text(left-8,yy+4,str(tick),'tick','end')
    for pi,phase in enumerate(phases):
        xx=left+45+pi*95;text(xx,bottom+22,phase.replace('_',' '),'tick','middle')
        for ei,(engine,color) in enumerate(ENGINES):
            r=next(r for r in records if (r['engine'],r['phase'])==(engine,phase));a,b=r['pss_bytes']['range']
            mark(xx+(ei-1.5)*10,memory_y(r['pss_bytes']['mean']/1048576),memory_y(a/1048576),memory_y(b/1048576),color)
    text(30,485,'Dots: mean of 3 independent rounds/processes. Bars: observed min–max round range, not a confidence interval.')
    text(30,510,'Memory includes entire process and fixed runtime. Logical requested allocation, RSS and code bytes are reported separately. Synthetic success corpus only.')
    parts.append('</svg>')
    Path(path).write_text('\n'.join(parts)+'\n')


if __name__=='__main__':plot(json.loads(Path(sys.argv[1]).read_text()),sys.argv[2])
