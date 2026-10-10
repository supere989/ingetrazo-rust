//! Native pointer-motion modifier snapshots that egui's frame-final state loses.
//! Two retained bounded buffers; one cold shared handle, no copied event list.
use std::sync::{Arc,Mutex};
const LIMIT:usize=4096;
#[derive(Clone,Copy)]struct Sample{index:usize,pos:egui::Pos2,modifiers:egui::Modifiers}
#[derive(Default)]pub struct Timeline{pending:Vec<Sample>,current:Vec<Sample>,pending_refused:bool,current_refused:bool}
pub type Handle=Arc<Mutex<Timeline>>;
fn id()->egui::Id{egui::Id::new("native_pointer_motion_modifiers")}
pub fn install(ctx:&egui::Context)->Handle{let handle=Arc::new(Mutex::new(Timeline::default()));ctx.data_mut(|d|d.insert_temp(id(),handle.clone()));handle}
/// Retrieve before taking the egui input lock. Only clones the cold shared handle.
pub fn get(ctx:&egui::Context)->Option<Handle>{ctx.data(|d|d.get_temp(id()))}
impl Timeline{
 /// Called immediately after native event ingress, before later modifier events.
 pub fn record_appended(&mut self,input:&egui::RawInput,start:usize){
  if start>input.events.len(){self.pending_refused=true;return;}
  for(index,event)in input.events.iter().enumerate().skip(start){if let egui::Event::PointerMoved(pos)=event{
   if self.pending.len()==LIMIT||self.pending.last().is_some_and(|s|s.index>=index){self.pending_refused=true;return;}
   self.pending.push(Sample{index,pos:*pos,modifiers:input.modifiers});
  }}
 }
 /// Swap existing storage once for each actually consumed native input batch.
 pub fn begin_frame(&mut self){std::mem::swap(&mut self.pending,&mut self.current);self.pending.clear();self.current_refused=self.pending_refused;self.pending_refused=false;}
 pub fn modifiers(&self,index:usize,pos:egui::Pos2)->Result<Option<egui::Modifiers>,&'static str>{
  if self.current_refused{return Err("Native pointer modifier batch exceeded bounded admission");}
  match self.current.binary_search_by_key(&index,|s|s.index){Ok(i)=>{let s=self.current[i];if s.pos!=pos{return Err("Native pointer modifier event identity mismatch");}Ok(Some(s.modifiers))},Err(_)=>Ok(None)}
 }
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn native_motion_keeps_snapshot_after_same_frame_release(){let mut t=Timeline::default();let mut input=egui::RawInput::default();input.modifiers.ctrl=true;input.events.push(egui::Event::PointerMoved(egui::pos2(1.,2.)));t.record_appended(&input,0);input.modifiers.ctrl=false;t.begin_frame();assert!(t.modifiers(0,egui::pos2(1.,2.)).unwrap().unwrap().ctrl);assert!(t.modifiers(0,egui::pos2(2.,2.)).is_err());t.begin_frame();assert!(t.modifiers(0,egui::pos2(1.,2.)).unwrap().is_none());}
 #[test]fn oversized_batch_refuses_then_recovers_without_unbounded_growth(){let mut t=Timeline::default();let mut input=egui::RawInput::default();input.events=vec![egui::Event::PointerMoved(egui::pos2(1.,2.));LIMIT+1];t.record_appended(&input,0);assert_eq!(t.pending.len(),LIMIT);t.begin_frame();assert!(t.modifiers(0,egui::pos2(1.,2.)).is_err());let capacity=t.current.capacity();t.begin_frame();assert!(t.modifiers(0,egui::pos2(1.,2.)).unwrap().is_none());assert_eq!(t.pending.capacity(),capacity);}
 #[test]fn warm_batches_reuse_both_buffers(){let mut t=Timeline::default();let mut input=egui::RawInput::default();input.events=vec![egui::Event::PointerMoved(egui::pos2(1.,2.));32];for _ in 0..2{t.record_appended(&input,0);t.begin_frame();}let capacities=(t.pending.capacity(),t.current.capacity());let pointers=(t.pending.as_ptr(),t.current.as_ptr());for _ in 0..100{t.record_appended(&input,0);t.begin_frame();}assert_eq!((t.pending.capacity(),t.current.capacity()),capacities);assert_eq!((t.pending.as_ptr(),t.current.as_ptr()),pointers);}

}
