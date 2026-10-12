#!/usr/bin/env python3
"""Strict original authoring comparison with named native radial-property correction."""
import argparse,copy,json,math,zipfile
from pathlib import Path
NAMES=['cotas-lifecycle', 'cotas_ang-lifecycle', 'cotas_rad-lifecycle', 'etiquetas-lifecycle', 'niveles-lifecycle', 'llamadas-lifecycle', 'cancel-degenerate', 'mixed-visibility-locked', 'remembered-style-frame', 'radial-diameter']
PAIRS=[('cotas-lifecycle', 'initial'), ('cotas-lifecycle', 'armed'), ('cotas-lifecycle', 'click-1'), ('cotas-lifecycle', 'click-2'), ('cotas-lifecycle', 'click-3'), ('cotas-lifecycle', 'select-tool'), ('cotas-lifecycle', 'property-one'), ('cotas-lifecycle', 'property-two'), ('cotas-lifecycle', 'property-noop'), ('cotas-lifecycle', 'move-press'), ('cotas-lifecycle', 'move-preview'), ('cotas-lifecycle', 'move-release'), ('cotas-lifecycle', 'undo-move'), ('cotas-lifecycle', 'redo-move'), ('cotas-lifecycle', 'delete'), ('cotas-lifecycle', 'undo-delete'), ('cotas-lifecycle', 'redo-delete'), ('cotas-lifecycle', 'undo-delete-final'), ('cotas-lifecycle', 'save'), ('cotas_ang-lifecycle', 'initial'), ('cotas_ang-lifecycle', 'armed'), ('cotas_ang-lifecycle', 'click-1'), ('cotas_ang-lifecycle', 'click-2'), ('cotas_ang-lifecycle', 'click-3'), ('cotas_ang-lifecycle', 'click-4'), ('cotas_ang-lifecycle', 'select-tool'), ('cotas_ang-lifecycle', 'property-one'), ('cotas_ang-lifecycle', 'property-two'), ('cotas_ang-lifecycle', 'property-noop'), ('cotas_ang-lifecycle', 'move-press'), ('cotas_ang-lifecycle', 'move-preview'), ('cotas_ang-lifecycle', 'move-release'), ('cotas_ang-lifecycle', 'undo-move'), ('cotas_ang-lifecycle', 'redo-move'), ('cotas_ang-lifecycle', 'delete'), ('cotas_ang-lifecycle', 'undo-delete'), ('cotas_ang-lifecycle', 'redo-delete'), ('cotas_ang-lifecycle', 'undo-delete-final'), ('cotas_ang-lifecycle', 'save'), ('cotas_rad-lifecycle', 'initial'), ('cotas_rad-lifecycle', 'armed'), ('cotas_rad-lifecycle', 'click-1'), ('cotas_rad-lifecycle', 'click-2'), ('cotas_rad-lifecycle', 'select-tool'), ('cotas_rad-lifecycle', 'property-one'), ('cotas_rad-lifecycle', 'property-two'), ('cotas_rad-lifecycle', 'property-noop'), ('cotas_rad-lifecycle', 'move-press'), ('cotas_rad-lifecycle', 'move-preview'), ('cotas_rad-lifecycle', 'move-release'), ('cotas_rad-lifecycle', 'undo-move'), ('cotas_rad-lifecycle', 'redo-move'), ('cotas_rad-lifecycle', 'delete'), ('cotas_rad-lifecycle', 'undo-delete'), ('cotas_rad-lifecycle', 'redo-delete'), ('cotas_rad-lifecycle', 'undo-delete-final'), ('cotas_rad-lifecycle', 'save'), ('etiquetas-lifecycle', 'initial'), ('etiquetas-lifecycle', 'armed'), ('etiquetas-lifecycle', 'click-1'), ('etiquetas-lifecycle', 'click-2'), ('etiquetas-lifecycle', 'select-tool'), ('etiquetas-lifecycle', 'property-one'), ('etiquetas-lifecycle', 'property-two'), ('etiquetas-lifecycle', 'property-noop'), ('etiquetas-lifecycle', 'move-press'), ('etiquetas-lifecycle', 'move-preview'), ('etiquetas-lifecycle', 'move-release'), ('etiquetas-lifecycle', 'undo-move'), ('etiquetas-lifecycle', 'redo-move'), ('etiquetas-lifecycle', 'delete'), ('etiquetas-lifecycle', 'undo-delete'), ('etiquetas-lifecycle', 'redo-delete'), ('etiquetas-lifecycle', 'undo-delete-final'), ('etiquetas-lifecycle', 'save'), ('niveles-lifecycle', 'initial'), ('niveles-lifecycle', 'armed'), ('niveles-lifecycle', 'click-1'), ('niveles-lifecycle', 'select-tool'), ('niveles-lifecycle', 'property-one'), ('niveles-lifecycle', 'property-two'), ('niveles-lifecycle', 'property-noop'), ('niveles-lifecycle', 'move-press'), ('niveles-lifecycle', 'move-preview'), ('niveles-lifecycle', 'move-release'), ('niveles-lifecycle', 'undo-move'), ('niveles-lifecycle', 'redo-move'), ('niveles-lifecycle', 'delete'), ('niveles-lifecycle', 'undo-delete'), ('niveles-lifecycle', 'redo-delete'), ('niveles-lifecycle', 'undo-delete-final'), ('niveles-lifecycle', 'save'), ('llamadas-lifecycle', 'initial'), ('llamadas-lifecycle', 'armed'), ('llamadas-lifecycle', 'click-1'), ('llamadas-lifecycle', 'click-2'), ('llamadas-lifecycle', 'select-tool'), ('llamadas-lifecycle', 'property-one'), ('llamadas-lifecycle', 'property-two'), ('llamadas-lifecycle', 'property-noop'), ('llamadas-lifecycle', 'move-press'), ('llamadas-lifecycle', 'move-preview'), ('llamadas-lifecycle', 'move-release'), ('llamadas-lifecycle', 'undo-move'), ('llamadas-lifecycle', 'redo-move'), ('llamadas-lifecycle', 'delete'), ('llamadas-lifecycle', 'undo-delete'), ('llamadas-lifecycle', 'redo-delete'), ('llamadas-lifecycle', 'undo-delete-final'), ('llamadas-lifecycle', 'save'), ('cancel-degenerate', 'initial'), ('cancel-degenerate', 'cota-armed'), ('cancel-degenerate', 'cota-first'), ('cancel-degenerate', 'cota-degenerate-second'), ('cancel-degenerate', 'cota-cancel-pending'), ('cancel-degenerate', 'cota-cancel-tool'), ('cancel-degenerate', 'angular-armed'), ('cancel-degenerate', 'angular-first'), ('cancel-degenerate', 'angular-second'), ('cancel-degenerate', 'angular-cancel-pending'), ('cancel-degenerate', 'angular-cancel-tool'), ('cancel-degenerate', 'radial-armed'), ('cancel-degenerate', 'radial-first'), ('cancel-degenerate', 'radial-zero'), ('cancel-degenerate', 'radial-cancel-tool'), ('cancel-degenerate', 'label-armed'), ('cancel-degenerate', 'label-first'), ('cancel-degenerate', 'label-cancel-pending'), ('cancel-degenerate', 'label-cancel-tool'), ('cancel-degenerate', 'level-armed'), ('cancel-degenerate', 'level-cancel-tool'), ('cancel-degenerate', 'callout-armed'), ('cancel-degenerate', 'callout-first'), ('cancel-degenerate', 'callout-degenerate-second'), ('cancel-degenerate', 'callout-cancel-pending'), ('cancel-degenerate', 'callout-cancel-tool'), ('cancel-degenerate', 'save'), ('mixed-visibility-locked', 'initial'), ('mixed-visibility-locked', 'locked-canvas-click'), ('mixed-visibility-locked', 'hidden-over-visible-click'), ('mixed-visibility-locked', 'list-select-locked'), ('mixed-visibility-locked', 'locked-property'), ('mixed-visibility-locked', 'locked-delete'), ('mixed-visibility-locked', 'undo-locked-delete'), ('mixed-visibility-locked', 'select-angular'), ('mixed-visibility-locked', 'select-radial'), ('mixed-visibility-locked', 'select-label'), ('mixed-visibility-locked', 'select-level'), ('mixed-visibility-locked', 'select-callout'), ('mixed-visibility-locked', 'save'), ('remembered-style-frame', 'initial'), ('remembered-style-frame', 'select-old-cota'), ('remembered-style-frame', 'remember-cota'), ('remembered-style-frame', 'clear-before-new-cota'), ('remembered-style-frame', 'new-cota-arm'), ('remembered-style-frame', 'new-cota-first'), ('remembered-style-frame', 'new-cota-second'), ('remembered-style-frame', 'new-cota-offset'), ('remembered-style-frame', 'select-tool'), ('remembered-style-frame', 'select-old-level'), ('remembered-style-frame', 'remember-level'), ('remembered-style-frame', 'clear-before-new-level'), ('remembered-style-frame', 'new-level-arm'), ('remembered-style-frame', 'new-level-place'), ('remembered-style-frame', 'undo-new-level'), ('remembered-style-frame', 'redo-new-level'), ('remembered-style-frame', 'save'), ('radial-diameter', 'initial'), ('radial-diameter', 'armed'), ('radial-diameter', 'centre'), ('radial-diameter', 'diameter'), ('radial-diameter', 'undo'), ('radial-diameter', 'redo'), ('radial-diameter', 'save')]
KEYS=['case','phase','composition','selection','undo','redo','mode','pending']
def equal(a,b,path='$'):
 assert type(a)is type(b),(path,'type',type(a).__name__,type(b).__name__)
 if isinstance(a,dict):
  assert a.keys()==b.keys(),(path,'keys',list(a),list(b))
  for key in a:equal(a[key],b[key],path+'.'+key)
 elif isinstance(a,list):
  assert len(a)==len(b),(path,'length',len(a),len(b))
  for i,(x,y)in enumerate(zip(a,b)):equal(x,y,path+'.'+str(i))
 else:
  if isinstance(a,float):assert math.isfinite(a)and math.isfinite(b),path
  assert a==b,(path,a,b)
def inventory(s,n):
 assert s['source_pin']=='6be29fe437a7cd992d4e079f71720821b25216b9'
 assert [c['name']for c in s['cases']]==NAMES and [c['name']for c in n['cases']]==NAMES
 assert [(r['case'],r['phase'])for r in s['states']]==PAIRS
 assert [(r['case'],r['phase'])for r in n['states']]==PAIRS
 for key in ['actual_egui','unknown_metadata']:assert n[key]is True,key
 assert [r['case']for r in n['controls']]==NAMES
 assert n['failures']==[]
 for control in n['controls']:
  assert control['idle_reuse']is True and control['placement_refusal']is True
  assert type(control['cache_builds'])is int and control['cache_builds']>=0
  if control['case'].endswith('-lifecycle'):assert control['cache_builds']>0
  eligible=control['case']!='cancel-degenerate'
  assert control['property_refusal']is (True if eligible else None)
  assert control['delete_persistence']is (True if eligible else None)
 for row in n['states']:
  assert type(row['undo'])is int and row['undo']>=0 and type(row['redo'])is int and row['redo']>=0
  if row['selection']is not None:assert set(row['selection'])=={'family','index'}and row['selection']['family']in ['cotas','cotas_ang','cotas_rad','etiquetas','niveles','llamadas']and type(row['selection']['index'])is int and row['selection']['index']>=0
  if 'callback_error_count'in row:assert type(row['callback_error_count'])is int and row['callback_error_count']==0

def expected_state(row):
 out={k:copy.deepcopy(row[k])for k in KEYS};corrected=row['case']=='cotas_rad-lifecycle'and row['phase']in [p for c,p in PAIRS if c=='cotas_rad-lifecycle'][5:]
 if corrected:
  out['undo']+=1
  if out['composition'].get('cotas_rad'):out['composition']['cotas_rad'][0]['text']='First <> café'if row['phase']=='property-one'else'Second <>'
 return out,corrected

def expected_readers(rows):
 result=copy.deepcopy(rows)
 for row in result:
  if row['case']=='cotas_rad-lifecycle':row['composition']['cotas_rad'][0]['text']='Second <>'
 return result

def expected_delete_readers(s):
 rows=[]
 for case in s['cases']:
  if case['name']=='cancel-degenerate':continue
  final=next(r for r in s['states']if r['case']==case['name']and r['phase']=='save');expected,_=expected_state(final);comp=expected['composition'];selection=expected['selection']
  if selection is None:
   selection=next({'family':family,'index':index}for family in ['cotas','cotas_ang','cotas_rad','etiquetas','niveles','llamadas']for index,item in enumerate(comp.get(family,[]))if not item.get('hidden',False))
  family=selection['family'];comp[family].pop(selection['index'])
  if not comp[family]:comp.pop(family)
  rows.append(dict(case=case['name'],composition=comp))
 return rows

def raw_document(path):
 data=path.read_bytes();return json.loads(data)if data[:2]!=b'PK'else json.loads(zipfile.ZipFile(path).read('document.json'))
def raw_guard(doc,paths):
 for path,value in paths.items():
  actual=doc
  for key in path.split('.'):actual=actual[int(key)]if isinstance(actual,list)else actual[key]
  equal(value,actual,'raw.'+path)

def verify_states(s,n):
 inventory(s,n);counts={'literal_source':0,'native_corrected_radial':0}
 for source,native in zip(s['states'],n['states']):
  expected,corrected=expected_state(source);equal(expected,{k:native[k]for k in KEYS},source['case']+'.'+source['phase']);counts['native_corrected_radial'if corrected else'literal_source']+=1
 return counts

def main():
 p=argparse.ArgumentParser();p.add_argument('source',type=Path);p.add_argument('native',type=Path);p.add_argument('comparison',type=Path);a=p.parse_args();s=json.loads((a.source/'original-annotations.json').read_text());n=json.loads((a.native/'native-annotations.json').read_text());report={'result':'pending','scope':'161 literal source states plus13 explicitly specified native radial-property correctness states; original radial UI remains broken and records unchanged. No handle/chained/multiselect/physical pixel claim.'};a.comparison.parent.mkdir(parents=True,exist_ok=True)
 def write():a.comparison.write_text(json.dumps(report,indent=2)+'\n')
 try:
  report['states']=verify_states(s,n)
  sr=json.loads((a.source/'original-reopens.json').read_text());nr=json.loads((a.native/'native-reopens.json').read_text());assert [r['case']for r in sr]==NAMES and [r['case']for r in nr]==NAMES;equal(expected_readers(sr),nr,'actual-original-readers');report['original_reader_pairs']=len(nr)
  raw=[]
  for guard in s['opaque_guards']:
   case=next(c for c in n['cases']if c['name']==guard['case']);path=(a.native/case['saved']).resolve();assert path.is_relative_to(a.native.resolve());doc=raw_document(path);raw_guard(doc,guard['paths']);raw.append((doc,guard['paths']))
  deleted=json.loads((a.native/'native-delete-reopens.json').read_text());equal(expected_delete_readers(s),deleted,'actual-original-delete-readers');report['original_delete_reader_pairs']=len(deleted)
  report['raw_guards']=sum(len(paths)for doc,paths in raw)
  negatives=[]
  def reject(name,op):
   try:op()
   except (AssertionError,KeyError,IndexError,TypeError,ValueError):negatives.append(name)
   else:raise AssertionError('Accepted mutation '+name)
  for name,change in [('missing-case',lambda d:d['cases'].pop()),('missing-state',lambda d:d['states'].pop()),('duplicate-state',lambda d:d['states'].__setitem__(-1,d['states'][0])),('reordered-states',lambda d:d['states'].reverse()),('wrong-family',lambda d:d['states'][4]['selection'].update(family='frames')),('bool-index',lambda d:d['states'][4]['selection'].update(index=False)),('bool-history',lambda d:d['states'][0].update(undo=False)),('false-egui',lambda d:d.update(actual_egui=False)),('false-unknown',lambda d:d.update(unknown_metadata=False)),('false-refusal',lambda d:d['controls'][0].update(placement_refusal=False)),('false-warm',lambda d:d['controls'][0].update(idle_reuse=False)),('missing-control',lambda d:d['controls'].pop()),('bool-cache-builds',lambda d:d['controls'][0].update(cache_builds=True)),('native-failure',lambda d:d['failures'].append({'case':NAMES[0],'error':'retained failure'})),('false-property-refusal',lambda d:d['controls'][7].update(property_refusal=False)),('false-delete-persistence',lambda d:d['controls'][8].update(delete_persistence=False)),('ineligible-property-claim',lambda d:d['controls'][6].update(property_refusal=True))]:
   candidate=copy.deepcopy(n);change(candidate);reject(name,lambda candidate=candidate:verify_states(s,candidate))
  for family in ['cotas','cotas_ang','cotas_rad','etiquetas','niveles','llamadas']:
   candidate=copy.deepcopy(n);row=next(r for r in candidate['states']if r['composition'].get(family));row['composition'].pop(family);reject('lost-'+family,lambda candidate=candidate:verify_states(s,candidate))
  for phase in ['property-one','property-two','property-noop','move-release','undo-move','redo-move','undo-delete-final','save']:
   candidate=copy.deepcopy(n);row=next(r for r in candidate['states']if r['case']=='cotas_rad-lifecycle'and r['phase']==phase);row['undo']-=1;reject('radial-corrected-history-'+phase,lambda candidate=candidate:verify_states(s,candidate))
  candidate=copy.deepcopy(n);row=next(r for r in candidate['states']if r['case']=='cotas_rad-lifecycle'and r['phase']=='property-two');row['composition']['cotas_rad'][0]['radius_mm']+=.001;reject('radial-unrelated-radius-change',lambda:verify_states(s,candidate))
  weak=json.loads((a.source/'failures/first-capture.partial.json').read_text());candidate=copy.deepcopy(n);old={(r['case'],r['phase']):r for r in weak['states']if r['case']=='etiquetas-lifecycle'}
  for index,row in enumerate(candidate['states']):
   if row['case']=='etiquetas-lifecycle':candidate['states'][index]={k:copy.deepcopy(old[(row['case'],row['phase'])][k])for k in KEYS}
  reject('retained-source-weak-label-body-hit',lambda:verify_states(s,candidate))
  candidate=copy.deepcopy(nr);candidate.pop();reject('reader-truncated',lambda:equal(expected_readers(sr),candidate))
  candidate=copy.deepcopy(nr);next(r for r in candidate if r['case']=='cotas_rad-lifecycle')['composition']['cotas_rad'][0]['text']='';reject('reader-lost-corrected-radial-text',lambda:equal(expected_readers(sr),candidate))
  candidate=copy.deepcopy(deleted);candidate.pop();reject('delete-reader-truncated',lambda:equal(expected_delete_readers(s),candidate))
  candidate=copy.deepcopy(deleted);candidate[0]['composition']['cotas']=copy.deepcopy(expected_state(next(r for r in s['states']if r['case']==NAMES[0]and r['phase']=='save'))[0]['composition']['cotas']);reject('delete-reader-resurrected-family',lambda:equal(expected_delete_readers(s),candidate))
  for doc,paths in raw:
   candidate=copy.deepcopy(doc);candidate['scene']['compositions'][0].pop('vendor_annotation_sheet');reject('lost-raw-sheet',lambda:raw_guard(candidate,paths))
   candidate=copy.deepcopy(doc);candidate['scene']['compositions'][0]['cotas_rad'][0]['vendor_annotation']['opaque'][0]=True;reject('raw-bool-int-alias',lambda:raw_guard(candidate,paths))
  report.update(result='pass',cases=len(NAMES),negative_controls=negatives)
 except (AssertionError,KeyError,IndexError,TypeError,ValueError,OSError)as error:report.update(result='fail',error=repr(error));write();print(json.dumps(report));return 1
 write();print(json.dumps(report));return 0
if __name__=='__main__':raise SystemExit(main())
