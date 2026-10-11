//! Native pointer-motion modifier snapshots that egui's frame-final state loses.
//! Two retained bounded buffers; one cold shared handle, no copied event list.
use std::sync::{Arc,Mutex};
const LIMIT:usize=4096;
#[derive(Clone,Copy)]struct Sample{index:usize,pos:egui::Pos2,modifiers:egui::Modifiers}
#[derive(Clone,Copy)]struct Change{index:usize,modifiers:egui::Modifiers}
#[derive(Default)]pub struct Timeline{pending:Vec<Sample>,current:Vec<Sample>,pending_changes:Vec<Change>,current_changes:Vec<Change>,last_modifiers:egui::Modifiers,pending_refused:bool,current_refused:bool}
pub type Handle=Arc<Mutex<Timeline>>;
fn id()->egui::Id{egui::Id::new("native_pointer_motion_modifiers")}
pub fn install(ctx:&egui::Context)->Handle{let handle=Arc::new(Mutex::new(Timeline::default()));ctx.data_mut(|d|d.insert_temp(id(),handle.clone()));handle}
/// Retrieve before taking the egui input lock. Only clones the cold shared handle.
pub fn get(ctx:&egui::Context)->Option<Handle>{ctx.data(|d|d.get_temp(id()))}
impl Timeline{
 /// Called immediately after native event ingress, before later modifier events.
 pub fn record_appended(&mut self,input:&egui::RawInput,start:usize){
  if start>input.events.len(){self.pending_refused=true;return;}
  if input.modifiers!=self.last_modifiers{
   if self.pending_changes.len()==LIMIT{self.pending_refused=true;return;}
   self.pending_changes.push(Change{index:start,modifiers:input.modifiers});self.last_modifiers=input.modifiers;
  }
  for(index,event)in input.events.iter().enumerate().skip(start){if let egui::Event::PointerMoved(pos)=event{
   if self.pending.len()==LIMIT||self.pending.last().is_some_and(|s|s.index>=index){self.pending_refused=true;return;}
   self.pending.push(Sample{index,pos:*pos,modifiers:input.modifiers});
  }}
 }
 /// Swap existing storage once for each actually consumed native input batch.
 pub fn begin_frame(&mut self){std::mem::swap(&mut self.pending,&mut self.current);self.pending.clear();std::mem::swap(&mut self.pending_changes,&mut self.current_changes);self.pending_changes.clear();self.current_refused=self.pending_refused;self.pending_refused=false;}
 pub fn admitted(&self)->bool{!self.current_refused}
 /// Deliver native modifier transitions in event order, including zero-event taps.
 /// Caller retains one cursor per consumed frame; no copied event list/allocation.
 pub fn visit_changes_before(&self,index:usize,cursor:&mut usize,mut visit:impl FnMut(egui::Modifiers))->Result<(),&'static str>{
  if self.current_refused{return Err("Native modifier batch exceeded bounded admission");}
  while let Some(change)=self.current_changes.get(*cursor){if change.index>index{break;}visit(change.modifiers);*cursor+=1;}
  Ok(())
 }
 pub fn modifiers(&self,index:usize,pos:egui::Pos2)->Result<Option<egui::Modifiers>,&'static str>{
  if self.current_refused{return Err("Native pointer modifier batch exceeded bounded admission");}
  match self.current.binary_search_by_key(&index,|s|s.index){Ok(i)=>{let s=self.current[i];if s.pos!=pos{return Err("Native pointer modifier event identity mismatch");}Ok(Some(s.modifiers))},Err(_)=>Ok(None)}
 }
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn native_motion_keeps_snapshot_after_same_frame_release(){let mut t=Timeline::default();let mut input=egui::RawInput::default();input.modifiers.ctrl=true;input.events.push(egui::Event::PointerMoved(egui::pos2(1.,2.)));t.record_appended(&input,0);input.modifiers.ctrl=false;t.begin_frame();assert!(t.modifiers(0,egui::pos2(1.,2.)).unwrap().unwrap().ctrl);assert!(t.modifiers(0,egui::pos2(2.,2.)).is_err());t.begin_frame();assert!(t.modifiers(0,egui::pos2(1.,2.)).unwrap().is_none());}
 #[test]fn oversized_batch_refuses_then_recovers_without_unbounded_growth(){let mut t=Timeline::default();let mut input=egui::RawInput::default();input.events=vec![egui::Event::PointerMoved(egui::pos2(1.,2.));LIMIT+1];t.record_appended(&input,0);assert_eq!(t.pending.len(),LIMIT);t.begin_frame();assert!(t.modifiers(0,egui::pos2(1.,2.)).is_err());let capacity=t.current.capacity();t.begin_frame();assert!(t.modifiers(0,egui::pos2(1.,2.)).unwrap().is_none());assert_eq!(t.pending.capacity(),capacity);}
 #[test]fn warm_batches_reuse_both_buffers(){let mut t=Timeline::default();let mut input=egui::RawInput::default();input.events=vec![egui::Event::PointerMoved(egui::pos2(1.,2.));32];for _ in 0..2{t.record_appended(&input,0);t.begin_frame();}let capacities=(t.pending.capacity(),t.current.capacity());let pointers=(t.pending.as_ptr(),t.current.as_ptr());for _ in 0..100{t.record_appended(&input,0);t.begin_frame();}assert_eq!((t.pending.capacity(),t.current.capacity()),capacities);assert_eq!((t.pending.as_ptr(),t.current.as_ptr()),pointers);}
 #[test]fn zero_event_taps_preserve_order_and_do_not_rearm_held_state(){let mut t=Timeline::default();let mut input=egui::RawInput::default();for m in [egui::Modifiers::CTRL,egui::Modifiers::NONE,egui::Modifiers::SHIFT,egui::Modifiers::NONE]{input.modifiers=m;t.record_appended(&input,0);}t.begin_frame();let mut seen=vec![];let mut cursor=0;t.visit_changes_before(0,&mut cursor,|m|seen.push(m)).unwrap();assert_eq!(seen,[egui::Modifiers::CTRL,egui::Modifiers::NONE,egui::Modifiers::SHIFT,egui::Modifiers::NONE]);t.visit_changes_before(usize::MAX,&mut cursor,|_|panic!("duplicate delivery")).unwrap();t.begin_frame();let mut cursor=0;t.visit_changes_before(usize::MAX,&mut cursor,|_|panic!("rearmed held state")).unwrap();}
 #[test]fn warm_taps_reuse_both_change_buffers(){let mut t=Timeline::default();let mut input=egui::RawInput::default();for _ in 0..2{for m in [egui::Modifiers::CTRL,egui::Modifiers::NONE]{input.modifiers=m;t.record_appended(&input,0);}t.begin_frame();}let capacities=(t.pending_changes.capacity(),t.current_changes.capacity());let pointers=(t.pending_changes.as_ptr(),t.current_changes.as_ptr());for _ in 0..100{for m in [egui::Modifiers::CTRL,egui::Modifiers::NONE]{input.modifiers=m;t.record_appended(&input,0);}t.begin_frame();}assert_eq!((t.pending_changes.capacity(),t.current_changes.capacity()),capacities);assert_eq!((t.pending_changes.as_ptr(),t.current_changes.as_ptr()),pointers);}
 #[test]fn modifier_bound_refuses_before_callbacks_then_recovers(){let mut t=Timeline::default();let mut input=egui::RawInput::default();for i in 0..=LIMIT{input.modifiers.ctrl=i%2==0;t.record_appended(&input,0);}t.begin_frame();assert!(!t.admitted());assert!(t.visit_changes_before(usize::MAX,&mut 0,|_|panic!("partial rejected batch")).is_err());t.begin_frame();assert!(t.admitted());}
 #[test]fn transitions_follow_raw_event_watermarks(){let mut t=Timeline::default();let mut input=egui::RawInput::default();input.events.push(egui::Event::PointerMoved(egui::pos2(1.,2.)));t.record_appended(&input,0);input.modifiers.ctrl=true;t.record_appended(&input,1);input.events.push(egui::Event::PointerMoved(egui::pos2(2.,2.)));t.record_appended(&input,1);input.modifiers.ctrl=false;t.record_appended(&input,2);t.begin_frame();let mut cursor=0;let mut seen=vec![];t.visit_changes_before(0,&mut cursor,|m|seen.push(m.ctrl)).unwrap();assert!(seen.is_empty());t.visit_changes_before(1,&mut cursor,|m|seen.push(m.ctrl)).unwrap();assert_eq!(seen,[true]);t.visit_changes_before(2,&mut cursor,|m|seen.push(m.ctrl)).unwrap();assert_eq!(seen,[true,false]);}
}
