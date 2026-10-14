"""L logical workload counts, computed independently from the common input.
No counters added to speed paths. Counts source-level branches/arithmetic,
not hardware instructions, CPU utilization, or an internal-stage estimate.
"""
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'bench/results/l-final'


def count(case,n):
    result=dict(case=case,rows=n,quantity_positive=0,price_filter_hits=0,
                selected_rows=0,accumulator_multiplications=0,accumulator_additions=0,state_updates=0)
    previous=0
    for i in range(n):
        price,quantity=(i*37)%1000,i%9-2
        if quantity>0:
            result['quantity_positive']+=1
            if price<=700:
                result['price_filter_hits']+=1
                if case=='weighted' or price>=previous:
                    result['selected_rows']+=1
                    if case=='stateful':previous=price;result['state_updates']+=1
    result['accumulator_multiplications']=result['accumulator_additions']=result['selected_rows']
    result['selected_fraction']=result['selected_rows']/n if n else None
    return result


def main():
    rows=[count(c,n) for c in ['weighted','stateful'] for n in [0,100,10000,100000]]
    by={(r['case'],r['rows']):r for r in rows}
    plan=json.loads((OUT/'plan.json').read_text())
    mixes=[]
    for mix in ['light','scan']:
        totals=dict(mix=mix,requests=40,visited_rows=0,selected_rows=0,
                    accumulator_multiplications=0,accumulator_additions=0,state_updates=0)
        for (case,n),weight in zip(plan['mixed']['shapes'],plan['mixed'][mix+'_weights']):
            if case=='scalar':continue
            r=by[(case,n)]
            totals['visited_rows']+=n*weight
            for metric in ['selected_rows','accumulator_multiplications','accumulator_additions','state_updates']:
                totals[metric]+=r[metric]*weight
        mixes.append(totals)
    result={'scope':'Offline deterministic source-level accounting for the actual successful input corpus; not instrumented timing/hardware-counter evidence',
            'producer':{'price':'(i*37)%1000','quantity':'i%9-2','price_limit':700,'batch_id':17},
            'rows':rows,'mixes':mixes,
            'exclusions':'Receipt +batch and scalar arithmetic not counted in accumulator operation columns; input materialization and predicate execution still cost CPU even for rejected rows'}
    (OUT/'workload-accounting.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result['rows'][-1],indent=2))


if __name__=='__main__':main()
