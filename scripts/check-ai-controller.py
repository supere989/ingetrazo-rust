#!/usr/bin/env python3
"""Validate the opaque standalone reset/discard controller, not run_python parity."""
import copy,json,sys
from pathlib import Path

def validate(data):
    assert data['production'] is False and data['wall_time_bound'] is False
    assert data['error_traceback_protocol_proven'] is False
    assert data['private_vm_no_reset_api'] is True
    cases=data['cases']
    assert [x['name'] for x in cases]==['success','noop','failure','caught-poison','next-session','prepare-refusal','retry']
    for x in cases:
        terminal='discarded' if x.get('error',False) else 'committed'
        assert x['events']==['armed','staged',terminal,'reset'],x
    assert [(x['vertices'],x['edges'],x['history']) for x in cases[:5]]==[(2,1,[1,0]),(2,1,[1,0]),(2,1,[1,0]),(2,1,[1,0]),(4,2,[2,0])]
    assert [x['poisoned'] for x in cases[:5]]==[False,False,False,True,False]
    assert cases[5]['edges']==2 and cases[6]['edges']==3
    assert cases[5]['error'] is True and cases[6]['changed'] is True

def main():
    data=json.loads(Path(sys.argv[1]).read_text());validate(data)
    failures=[]
    for name,mutate in [
        ('early-reset',lambda d:d['cases'][3].update(events=['armed','staged','reset','discarded'])),
        ('partial-publication',lambda d:d['cases'][2].update(edges=2)),
        ('poison-lost',lambda d:d['cases'][3].update(poisoned=False)),
        ('missing-case',lambda d:d['cases'].pop()),
    ]:
        broken=copy.deepcopy(data);mutate(broken)
        try:validate(broken)
        except (AssertionError,KeyError,IndexError):failures.append(name)
        else:raise AssertionError('Accepted mutation: '+name)
    print(json.dumps({'cases':len(data['cases']),'negative_rejections':failures,'production':False},indent=2))
if __name__=='__main__':main()
