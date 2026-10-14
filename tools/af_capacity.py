"""AF open-loop capacity campaign. Driver CPU4, service tree CPU2, no SMT sharing.
Queue/admission live in this external harness, not in production runtimes.
python3 af_capacity.py BASE smoke|pilot|confirm ROUND
"""
from collections import deque, Counter
import gzip
import hashlib
import json
import math
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys
import time

ARMS = ['interpreter', 'native', 'monty', 'fork', 'cow']
SHAPES = [('scalar', 0)] * 24 + [('weighted', 100)] * 8 + [('stateful', 100)] * 4 + [('weighted', 10000)] * 2 + [('stateful', 100000)] * 2
COHORT = 'af-open-loop-v1'
QUEUE = 64
INFLIGHT = 16


def shape(workload, i):
    return ('weighted', 10000) if workload == 'cpu' else SHAPES[(i * 17) % 40]


def quantile(values, p):
    if not values: return None
    values = sorted(values)
    return values[max(0, math.ceil(p * len(values)) - 1)]


def summarize(rows, trace, meta):
    measurement_start = meta['start_ns'] + int(meta['warmup_s'] * 1e9)
    measurement_end = meta['stop_ns']
    selected = [r for r in rows if measurement_start <= r['planned_ns'] < measurement_end]
    done = [r for r in selected if r['status'] == 'completed']
    duration = max(0, (measurement_end-measurement_start)/1e9)
    window_completions = [r for r in rows if r['status'] == 'completed' and measurement_start <= r['response_ns'] < measurement_end]
    hist = Counter(r['status'] for r in rows)
    assert sum(hist.values()) == meta['offered'] == len(rows)
    def pct(field): return {f'p{p}': quantile([r[field] / 1e6 for r in done], p/100) for p in [50,95,99]}
    used_trace = [t for t in trace if measurement_start <= t['mono_ns'] <= measurement_end]
    first = [r['latency_ns']/1e6 for r in done if r['planned_ns'] < measurement_start+duration*1e9/3]
    last = [r['latency_ns']/1e6 for r in done if r['planned_ns'] >= measurement_start+duration*2e9/3]
    xs = [(t['mono_ns']-measurement_start)/1e9 for t in used_trace]
    ys = [t['backlog'] for t in used_trace]
    if len(xs)>1:
        xm,ym=sum(xs)/len(xs),sum(ys)/len(ys)
        slope = sum((x-xm)*(y-ym) for x,y in zip(xs,ys))/sum((x-xm)**2 for x in xs)
    else: slope=None
    completion_rate = len(window_completions)/duration if duration else None
    p99a,p99b=quantile(first,.99),quantile(last,.99)
    stable = (duration >= 8 and not meta['early_stop'] and hist['rejected']==hist['timeout']==hist['failed']==0
        and len(done)==len(selected) and completion_rate >= meta['rate']*.98
        and slope is not None and slope <= max(.5,meta['rate']*.01)
        and p99a is not None and p99b is not None and p99b <= p99a*1.5+2
        and (not ys or sum(ys[-3:])/len(ys[-3:]) <= max(2,meta['rate']*.05)))
    return {**meta,'counts':dict(hist),'admitted':sum(r['status']!='rejected' for r in rows),
        'measurement_s':duration,'measurement_offered':len(selected),'measurement_completed':len(done),
        'completion_rate':completion_rate,'drained_rate':len(done)/duration if duration else None,
        'latency_ms':pct('latency_ns'),'queue_ms':pct('queue_ns'),'service_ms':pct('service_ns'),
        'p99_first_third_ms':p99a,'p99_last_third_ms':p99b,'backlog_slope_per_s':slope,
        'max_backlog':max((t['backlog'] for t in trace),default=0),'stable_observed':stable,
        'service_cpu_ms_per_completed':meta['service_cpu_ns']/hist['completed']/1e6 if hist['completed'] and meta['service_cpu_ns'] is not None else None,
        'driver_cpu_ms_per_offered':meta['driver_cpu_ns']/len(rows)/1e6 if rows else None,
        'arrival_lateness_ms':{f'p{p}':quantile([r['offered_ns']-r['planned_ns'] for r in rows],p/100)/1e6 for p in [50,95,99]},
        'trace':trace}


def run(base, arm, workload, rate, duration, warmup, key):
    out=base/'af-capacity'; raw=out/(key+'.jsonl.gz'); summary=out/(key+'.json')
    if summary.exists(): return json.loads(summary.read_text())
    assert not raw.exists(), 'partial raw must be inspected, not overwritten'
    delay=0 if workload=='cpu' else 1000
    env={**os.environ,'MINIPY_COMMON':'1','MINIPY_TOOL_DELAY_US':str(delay),'AD_NATIVE_SCRIPT':str(base/'ad-corrected/native.py'),'GOMAXPROCS':'1'}
    if arm in ['fork','cow','resident']:
        argv=['taskset','-c','2',str(out/'worker'),arm,'weighted','capacity','10000','1',str(delay),str(base/'source/fixtures'),str(base/'w-guest/runtime/dist/pysolate.wasm')]
    else:
        argv=['taskset','-c','2',str(out/'interpreter_ab'),arm,'weighted','capacity','10000','1']
    log=open(out/(key+'.stderr'),'w')
    p=subprocess.Popen(argv,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,env=env,start_new_session=True)
    sel=selectors.DefaultSelector(); sel.register(p.stdout,selectors.EVENT_READ)
    os.set_blocking(p.stdout.fileno(),False); os.set_blocking(p.stdin.fileno(),False)
    buf=b''; ready=None; final=None
    def messages(timeout):
        nonlocal buf
        result=[]
        for _,_ in sel.select(timeout):
            b=os.read(p.stdout.fileno(),1<<20)
            if not b:
                if p.poll() is not None: raise RuntimeError(f'worker exited {p.returncode}: {out/(key+".stderr")}')
                continue
            buf+=b
            while b'\n' in buf:
                line,buf=buf.split(b'\n',1); result.append(json.loads(line))
        return result
    try:
        init_due=time.monotonic()+30
        while ready is None:
            for r in messages(.05):
                if r['kind']=='ready':ready=r
            if time.monotonic()>init_due:raise TimeoutError('runtime init')
        now=time.monotonic_ns(); assert 0 <= now-ready['mono_ns'] < 100_000_000, 'monotonic clock mismatch'
        start=now+100_000_000; nominal_stop=start+int(duration*1e9); stop=nominal_stop
        total=math.ceil(duration*rate); rows=[]; queue=deque(); active={}; trace=[]
        next_id=0; next_trace=start; sent_final=False; early=False; consecutive_full=0
        completed_count=0; rejected_count=0
        driver_start=time.process_time_ns(); final_due=None
        while final is None:
            now=time.monotonic_ns()
            while next_id<total and now>=start+round(next_id/rate*1e9) and start+round(next_id/rate*1e9)<stop:
                case,n=shape(workload,next_id); due=start+round(next_id/rate*1e9)
                row={'id':next_id,'case':case,'rows':n,'planned_ns':due,'offered_ns':now,'status':'queued'}
                rows.append(row);next_id+=1
                if len(queue)>=QUEUE:
                    row['status']='rejected';rejected_count+=1
                else:queue.append(row)
            while queue and len(active)<INFLIGHT:
                r=queue[0]; wire=json.dumps({k:r[k] for k in ['id','case','rows','planned_ns']},separators=(',',':')).encode()+b'\n'
                dispatch=time.monotonic_ns()
                try: written=os.write(p.stdin.fileno(),wire)
                except BlockingIOError:break
                assert written==len(wire), 'atomic small pipe write'
                queue.popleft();r['status']='active';r['dispatch_ns']=dispatch;active[r['id']]=r
            if now>=next_trace and now>=start:
                trace.append({'mono_ns':now,'offered':len(rows),'completed':completed_count,
                    'rejected':rejected_count,'queued':len(queue),'inflight':len(active),'backlog':len(queue)+len(active)})
                # Bound overload duration, preserve offered accounting and drain.
                consecutive_full=consecutive_full+1 if len(queue)>=48 else 0
                if consecutive_full>=3 and now>=start+int((warmup+3)*1e9) and now<stop:
                    stop=now;early=True
                next_trace+=1_000_000_000
            if now>=stop and final_due is None:final_due=now+30_000_000_000
            if now>=stop and not queue and not active and not sent_final:
                os.write(p.stdin.fileno(),b'{"id":-1}\n');sent_final=True
            if final_due and now>=final_due:
                for r in list(queue)+list(active.values()):r['status']='timeout'
                break
            next_arrival=start+round(next_id/rate*1e9) if next_id<total and now<stop else now+10_000_000
            wait=max(0,min(.005,(next_arrival-time.monotonic_ns())/1e9))
            for msg in messages(wait):
                if msg['kind']=='result':
                    r=active.pop(msg['id']); response=time.monotonic_ns()
                    assert r['planned_ns']<=r['dispatch_ns']<=msg['start_ns']<=msg['end_ns']<=response
                    completed_count+=int(msg['ok'])
                    r.update(status='completed' if msg['ok'] else 'failed',stdout=msg['stdout'],
                        start_ns=msg['start_ns'],end_ns=msg['end_ns'],response_ns=response,
                        latency_ns=response-r['planned_ns'],queue_ns=msg['start_ns']-r['planned_ns'],service_ns=msg['end_ns']-msg['start_ns'])
                elif msg['kind']=='final':final=msg
        end=time.monotonic_ns();driver_cost=time.process_time_ns()-driver_start
        if final:
            expected_tools=sum(r['case']!='scalar' and r['status']=='completed' for r in rows)
            assert final['loads']==final['records']==expected_tools, (final,expected_tools)
            p.stdin.close();p.wait(timeout=5);assert p.returncode==0
        else:
            os.killpg(p.pid,signal.SIGKILL);p.wait()
        meta={'cohort':COHORT,'key':key,'arm':arm,'workload':workload,'rate':rate,'duration_s':duration,'warmup_s':warmup,
            'start_ns':start,'stop_ns':stop,'nominal_stop_ns':nominal_stop,'drain_end_ns':end,'early_stop':early,
            'offered':len(rows),'scheduled':total,'cancelled_unoffered':total-len(rows),'service_cpu_ns':final['cpu_ns'] if final else None,
            'driver_cpu_ns':driver_cost,'argv':argv,'delay_us':delay,'queue_limit':QUEUE,'inflight_limit':INFLIGHT,
            'driver_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'driver_revision':'constant-time-trace-v2','driver_affinity':sorted(os.sched_getaffinity(0)),'service_affinity':[2],'final':final}
        result=summarize(rows,trace,meta)
        with gzip.open(raw,'wt') as f:
            f.write(json.dumps({'kind':'meta',**meta})+'\n')
            for r in rows:f.write(json.dumps({'kind':'request',**r})+'\n')
            for t in trace:f.write(json.dumps({'kind':'trace',**t})+'\n')
        summary.write_text(json.dumps(result,indent=2)+'\n')
        return result
    finally:
        if p.poll() is None:os.killpg(p.pid,signal.SIGKILL);p.wait()
        sel.close();log.close()


def main(base, mode, round_id):
    out=base/'af-capacity';out.mkdir(exist_ok=True)
    assert os.sched_getaffinity(0)=={4}, 'driver must use separate physical core'
    if mode=='smoke':
        for arm in ARMS:
            for workload in ['cpu','mixed']:
                s=run(base,arm,workload,10 if arm=='cow' else 40,2,0,f'smoke-{arm}-{workload}')
                assert not s['counts'].get('failed',0) and not s['counts'].get('timeout',0)
                print(s['key'],s['counts'],s['service_cpu_ms_per_completed'],flush=True)
        return
    if mode=='pilot':
        plan=[]
        for workload in ['cpu','mixed']:
            for arm in ARMS:
                seeds={'cpu':{'interpreter':1600,'native':400,'monty':80,'fork':30,'cow':5},'mixed':{'interpreter':1600,'native':400,'monty':120,'fork':30,'cow':5}}
                rate=seeds[workload][arm];last_good=None;bad=None
                for step in range(5):
                    s=run(base,arm,workload,rate,4,1,f'pilot-{arm}-{workload}-{rate}')
                    print(s['key'],round(s['completion_rate'],2),s['counts'],s['latency_ms'],flush=True)
                    # Pilot only brackets throughput; no stability claim from 3 sec.
                    overload=s['counts'].get('rejected',0)>0 or s['completion_rate']<rate*.9 or s['max_backlog']>max(16,rate*.05)
                    if overload:bad=rate;break
                    last_good=rate;rate*=2
                if last_good is None:
                    rate=max(1,rate/2);s=run(base,arm,workload,rate,4,1,f'pilot-{arm}-{workload}-{rate:g}');last_good=rate
                if bad is None:bad=rate
                # Three points only: shared public load, lower bracket, upper bracket.
                rates=sorted(set([10.0 if workload=='mixed' else 4.0,float(last_good),float(bad)]))
                plan.append({'arm':arm,'workload':workload,'rates':rates,'pilot_last_good':last_good,'pilot_first_bad':bad})
        (out/'plan.json').write_text(json.dumps({'cohort':COHORT,'duration_s':15,'warmup_s':3,'cells':plan,'confirmation_rounds':3},indent=2)+'\n')
        return
    assert mode=='confirm'
    plan=json.loads((out/'plan.json').read_text());cells=plan['cells']
    order=cells[round_id:]+cells[:round_id]
    if round_id%2:order=order[::-1]
    for cell in order:
        if os.environ.get('AF_WORKLOAD') and cell['workload'] != os.environ['AF_WORKLOAD']:
            continue
        for rate in cell['rates']:
            key=f'confirm-{round_id}-{cell["arm"]}-{cell["workload"]}-{rate:g}'
            s=run(base,cell['arm'],cell['workload'],rate,plan['duration_s'],plan['warmup_s'],key)
            print(key,'completed',s['counts'],'rate',s['completion_rate'],'stable',s['stable_observed'],flush=True)


if __name__=='__main__':main(Path(sys.argv[1]).resolve(),sys.argv[2],int(sys.argv[3]) if len(sys.argv)>3 else 0)
