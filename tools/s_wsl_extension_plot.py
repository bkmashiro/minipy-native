"""S bounded extension figures, stdlib SVG only. SUMMARY OUTPUT_DIRECTORY.
Log-scale line positions use actual row counts, never equal categorical spacing.
Bars are observed round ranges, not confidence intervals. No missing-to-zero.
"""
import html
import json
import math
from pathlib import Path
import sys

COLORS={'native':'#2563eb','interpreter':'#d97706','monty':'#dc2626','reference':'#64748b',
        'cpython-resident':'#16a34a','cpython-prepared':'#16a34a','cpython-spawn':'#7c3aed',
        'cpython-fork':'#0891b2','pypy-resident':'#be185d','pypy-prepared':'#be185d',
        'pyodide-resident':'#92400e','pyodide-prepared':'#92400e','pysolate-fresh':'#111827','pysolate-cow':'#6b7280',
        'assembly':'#059669','rust':'#64748b'}


class SVG:
    def __init__(self,w,h,title):
        self.w=w;self.h=h
        self.parts=[f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">',f'<rect width="{w}" height="{h}" fill="white"/>',
                    '<style>text{font:14px sans-serif;fill:#172033}.small{font-size:12px}.title{font-size:20px;font-weight:600}.panel{font-size:16px;font-weight:600}</style>']
        self.text(30,30,title,'title')
    def text(self,x,y,t,cls='',anchor='start'):
        self.parts.append(f'<text x="{x}" y="{y}" class="{cls}" text-anchor="{anchor}">{html.escape(str(t))}</text>')
    def line(self,x,y,a,b,color='#d9dfe8',width=1):
        self.parts.append(f'<line x1="{x}" y1="{y}" x2="{a}" y2="{b}" stroke="{color}" stroke-width="{width}"/>')
    def rect(self,x,y,w,h,color):
        self.parts.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{color}"/>')
    def save(self,path):Path(path).write_text('\n'.join(self.parts+['</svg>'])+'\n')


def lines(summary,path,title,arms,reference=False):
    g=SVG(1240,890,title)
    g.text(30,55,'Linux x86_64 WSL · input rows and request wall µs, both log scale · three independent rounds per point')
    g.text(30,78,'Filled dots: original retained cohort; hollow dots: extension. No repeated point is pooled across cohorts. Ranges: round min–max.','small')
    all_arms=list(dict.fromkeys(e for group in arms for e in group))
    for i,e in enumerate(all_arms):
        col=i%5;row=i//5
        g.line(30+col*240,105+row*22,48+col*240,105+row*22,COLORS[e],3)
        g.text(55+col*240,110+row*22,e,'small')
    top0=150+(len(all_arms)-1)//5*22
    modes=['kernel','full'] if reference else ['cold','warm']
    for ci,case in enumerate(['weighted','stateful']):
        for mi,mode in enumerate(modes):
            left=80+mi*610;top=top0+ci*305;bottom=top+220;width=480
            g.text(left,top-17,case+' · '+mode+' · '+('kernel-only' if mode=='kernel' else 'full request' if mode=='full' else 'prepare + execute' if mode=='cold' else 'prepared execute'),'panel')
            source=summary['reference'] if reference else summary['curve']
            records=[r for r in source if r['case']==case and r['mode']==mode and r['engine'] in arms[mi]]
            vals=[v for r in records for v in r['round_ranges']['wall_mean_us'] if v>0]
            low=math.floor(math.log10(min(vals)));high=math.ceil(math.log10(max(vals)))
            if high==low:high+=1
            def x(n):return left+(math.log10(n)-2)/3*width
            def y(v):return bottom-(math.log10(v)-low)/(high-low)*(bottom-top)
            for exponent in range(low,high+1):
                yy=y(10**exponent);g.line(left,yy,left+width,yy);g.text(left-8,yy+4,f'{10**exponent:g}','small','end')
            for n in [100,300,1000,3000,10000,30000,100000]:
                xx=x(n);g.line(xx,top,xx,bottom,'#eef1f5');g.text(xx,bottom+20,f'{n//1000}k' if n>=1000 else str(n),'small','middle')
            for e in arms[mi]:
                rs=sorted([r for r in records if r['engine']==e],key=lambda r:r['rows'])
                # No interpolation over an unsupported interior observation.
                for a,b in zip(rs,rs[1:]):g.line(x(a['rows']),y(a['wall_mean_us']),x(b['rows']),y(b['wall_mean_us']),COLORS[e],2)
                for r in rs:
                    xx=x(r['rows']);a,b=r['round_ranges']['wall_mean_us']
                    g.line(xx,y(a),xx,y(b),COLORS[e],2)
                    g.line(xx-3,y(a),xx+3,y(a),COLORS[e],2);g.line(xx-3,y(b),xx+3,y(b),COLORS[e],2)
                    fill=COLORS[e] if r['cohort']=='original' else 'white'
                    g.parts.append(f'<circle cx="{xx}" cy="{y(r["wall_mean_us"])}" r="4" fill="{fill}" stroke="{COLORS[e]}" stroke-width="2"/>')
            if 'pysolate-fresh' in arms[mi]:g.text(left,bottom+43,'Old Guest: 30k/100k unsupported (1 MiB Tool ABI); lines stop at 10k.','small')
            if case=='stateful':g.text(left,bottom+62,'Predicate-dominated generated input, not an arithmetic-heavy workload.','small')
    g.text(30,845,'Kernel-only excludes input construction/validation, Tools, output and async dispatch. Full-request includes them. Rust/asm are fixed-fixture floors.','small')
    g.text(30,868,'Ordinary runtimes differ in object representation, arithmetic, lifecycle and authority. Curves do not establish general Python or sandbox equivalence.','small')
    g.save(path)


def lifecycle(summary,path):
    engines=['native','interpreter','monty','cpython','pypy','pyodide-node','pysolate-fresh','pysolate-cow']
    phases=[('process_startup_ns','#94a3b8','launch→entry'),('runtime_init_ns','#7c3aed','runtime/harness init'),('source_prepare_ns','#d97706','source prepare'),('execute_ns','#16a34a','first execution'),('reap_unassigned_ns','#dc2626','response/reap/unassigned')]
    g=SVG(1400,990,'S · same-run first request: fresh process launch through response and reap')
    g.text(30,56,'Each bar sums mean phase timings from the same three processes; total range is observed min–max, not a confidence interval.')
    g.text(30,80,'Linear wall ms; each engine has its own labelled scale. Resident/prepared/fork observations stay outside this fresh-process chart.','small')
    for i,(_,color,label) in enumerate(phases):g.rect(30+i*270,99,12,12,color);g.text(48+i*270,110,label,'small')
    for i,e in enumerate(engines):
        col=i%2;row=i//2;left=160+col*690;top=165+row*185;width=485
        records=[r for r in summary['first'] if r['engine']==e]
        maximum=max(r['metrics']['wall_ns']['range_ms'][1] for r in records)*1.1
        g.text(left-120,top-20,e,'panel')
        for k in range(6):
            xx=left+width*k/5;g.line(xx,top-5,xx,top+100,'#eef1f5');g.text(xx,top+120,f'{maximum*k/5:.2f}','small','middle')
        g.text(left+width,top+143,'wall ms','small','end')
        for j,(case,n) in enumerate([('scalar',0),('weighted',10000),('stateful',10000)]):
            r=next(r for r in records if (r['case'],r['rows'])==(case,n));yy=top+j*33;offset=0
            g.text(left-10,yy+15,case+(' 10k' if n else ''),'small','end')
            for phase,color,_ in phases:
                size=r['metrics'][phase]['mean_ms']/maximum*width
                g.rect(left+offset,yy,size,20,color);offset+=size
            a,b=r['metrics']['wall_ns']['range_ms'];g.line(left+a/maximum*width,yy+24,left+b/maximum*width,yy+24,'#111827',2)
    g.text(30,945,'Old Pysolate source stage is Host adaptation only. Guest Python compilation and instance initialization remain inside Runner.Run execution.','small')
    g.text(30,968,'Runtime init includes first imports/setup; Rust source preparation includes compile. Pyodide includes real Wasm load/instantiate + Python harness imports.','small')
    g.save(path)


def main(s,out):
    out.mkdir(parents=True,exist_ok=True)
    lines(s,out/'scale-primary.svg','S · seven-point primary source-preparation and prepared execution curves', [['native','interpreter','monty','reference']]*2)
    lines(s,out/'scale-python.svg','S · seven-point ordinary-runtime lifecycle observations', [['cpython-resident','pypy-resident','pyodide-resident','cpython-fork','cpython-spawn','pysolate-fresh','pysolate-cow'],['cpython-prepared','pypy-prepared','pyodide-prepared','native','interpreter']])
    lines(s,out/'reference-x86.svg','S · true handwritten x86_64 assembly versus handwritten Rust, paired reference cohort', [['assembly','rust']]*2,True)
    lifecycle(s,out/'first-request.svg')


if __name__=='__main__':main(json.loads(Path(sys.argv[1]).read_text()),Path(sys.argv[2]))
