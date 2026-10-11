"""Focused dictionary identity/history prerequisite; not full weld authority."""
import copy,json,sys
COMMON={'persistent_dictionary','undo_redo_owned_contents','failed_command_restore','production_enabled'}
def validate(source,native):
 assert set(source)==COMMON
 assert set(native)==COMMON|{'final_charge'}
 for key in COMMON:assert type(source[key]) is bool and type(native[key]) is bool and source[key]==native[key]
 assert source['production_enabled'] is False
 assert all(source[k] is True for k in COMMON-{'production_enabled'})
 assert type(native['final_charge']) is int and native['final_charge']==0
s=json.load(open(sys.argv[1]));n=json.load(open(sys.argv[2]));validate(s,n)
rejected=[]
for k in n:
 x=copy.deepcopy(n);x[k]=None
 try:validate(s,x)
 except (AssertionError,KeyError):rejected.append(k)
 else:raise AssertionError('accepted mutation '+k)
x=copy.deepcopy(s);x.pop('persistent_dictionary')
try:validate(x,n)
except (AssertionError,KeyError):rejected.append('source inventory')
else:raise AssertionError('accepted missing source control')
print(json.dumps({'matched_controls':3,'rejected_controls':rejected,'production_enabled':False}))
