//! CPU range ownership planner; no GPU upload or renderer activation.
//! Pending ranges are unpublished and retired ranges remain unavailable until
//! their actual graphics completion serial is acknowledged.
use std::collections::BTreeMap;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Range{pub page:u64,pub offset:u64,pub bytes:u64}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub struct Ticket{pub id:u64,pub range:Range}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]enum Owner{Pending,Published{last_use:u64},Retired{after:u64}}
struct Allocation{range:Range,owner:Owner}
struct Page{bytes:u64,free:BTreeMap<u64,u64>}
#[derive(Default)]pub struct Planner{pages:BTreeMap<u64,Page>,allocations:BTreeMap<u64,Allocation>,next:u64,completed:u64}
#[derive(Debug,Default,PartialEq,Eq)]pub struct Metrics{pub page_bytes:u64,pub pending_bytes:u64,pub published_bytes:u64,pub retired_bytes:u64}
impl Planner{
 pub fn add_page(&mut self,id:u64,bytes:u64)->Result<(),String>{if bytes==0||self.pages.contains_key(&id){return Err("Invalid or duplicate arena page".into());}self.pages.insert(id,Page{bytes,free:BTreeMap::from([(0,bytes)])});Ok(())}
 /// Only fully free pages qualify; pending and unacknowledged retired owners exclude them.
 pub fn empty_pages(&self)->impl Iterator<Item=(u64,u64)>+'_ {self.pages.iter().filter_map(|(&id,p)|(p.free.len()==1&&p.free.get(&0)==Some(&p.bytes)).then_some((id,p.bytes)))}
 pub fn remove_empty_page(&mut self,id:u64)->Result<(),String>{if self.allocations.values().any(|a|a.range.page==id){return Err("Arena page still owns ranges".into());}self.pages.remove(&id).ok_or("Unknown arena page")?;Ok(())}
 pub fn reserve(&mut self,bytes:u64,alignment:u64)->Result<Option<Ticket>,String>{
  if bytes==0||alignment==0{return Err("Arena reservation needs nonzero size/alignment".into());}
  let id=self.next.checked_add(1).ok_or("Arena ticket exhausted")?;
  let mut candidate=None;
  for(&page,p)in &self.pages{for(&offset,&extent)in &p.free{let Some(start)=offset.checked_add(alignment-1).map(|v|v/alignment*alignment)else{continue;};let Some(end)=start.checked_add(bytes)else{continue;};if end<=offset+extent{candidate=Some((page,offset,extent,start,end));break;}}if candidate.is_some(){break;}}
  let Some((page,offset,extent,start,end))=candidate else{return Ok(None);};let p=self.pages.get_mut(&page).unwrap();p.free.remove(&offset);if start>offset{p.free.insert(offset,start-offset);}if end<offset+extent{p.free.insert(end,offset+extent-end);}
  let range=Range{page,offset:start,bytes};self.allocations.insert(id,Allocation{range,owner:Owner::Pending});self.next=id;Ok(Some(Ticket{id,range}))
 }
 fn allocation(&mut self,ticket:Ticket)->Result<&mut Allocation,String>{let a=self.allocations.get_mut(&ticket.id).ok_or("Unknown arena ticket")?;if a.range!=ticket.range{return Err("Arena ticket range mismatch".into());}Ok(a)}
 pub fn publish(&mut self,ticket:Ticket)->Result<(),String>{let a=self.allocation(ticket)?;if a.owner!=Owner::Pending{return Err("Only unpublished arena ranges may publish".into());}a.owner=Owner::Published{last_use:0};Ok(())}
 pub fn mark_used(&mut self,ticket:Ticket,serial:u64)->Result<(),String>{if serial<=self.completed{return Err("Arena submission serial is already completed".into());}let a=self.allocation(ticket)?;let Owner::Published{last_use}=a.owner else{return Err("Cannot draw pending or retired arena range".into());};a.owner=Owner::Published{last_use:last_use.max(serial)};Ok(())}
 pub fn retire(&mut self,ticket:Ticket)->Result<(),String>{let a=self.allocation(ticket)?;let Owner::Published{last_use}=a.owner else{return Err("Only published ranges may retire".into());};a.owner=Owner::Retired{after:last_use};Ok(())}
 pub fn rollback(&mut self,ticket:Ticket)->Result<(),String>{if self.allocation(ticket)?.owner!=Owner::Pending{return Err("Cannot rollback a published arena range".into());}self.release(ticket.id);Ok(())}
 fn release(&mut self,id:u64){let a=self.allocations.remove(&id).unwrap();let p=self.pages.get_mut(&a.range.page).unwrap();let(mut offset,mut bytes)=(a.range.offset,a.range.bytes);if let Some((&before,&len))=p.free.range(..offset).next_back(){if before+len==offset{p.free.remove(&before);offset=before;bytes+=len;}}if let Some((&after,&len))=p.free.range(offset..).next(){if offset+bytes==after{p.free.remove(&after);bytes+=len;}}p.free.insert(offset,bytes);}
 /// Call only with the actual graphics fence's acknowledged serial. Unknown
 /// completion must not call this method, even after a wall-clock timeout.
 pub fn collect(&mut self,acknowledged:u64)->Result<usize,String>{if acknowledged<self.completed{return Err("Arena completion serial regressed".into());}self.completed=acknowledged;let ids=self.allocations.iter().filter_map(|(&id,a)|matches!(a.owner,Owner::Retired{after}if after<=acknowledged).then_some(id)).collect::<Vec<_>>();let count=ids.len();for id in ids{self.release(id);}Ok(count)}
 pub fn metrics(&self)->Metrics{let mut m=Metrics{page_bytes:self.pages.values().map(|p|p.bytes).sum(),..Default::default()};for a in self.allocations.values(){match a.owner{Owner::Pending=>m.pending_bytes+=a.range.bytes,Owner::Published{..}=>m.published_bytes+=a.range.bytes,Owner::Retired{..}=>m.retired_bytes+=a.range.bytes}}m}
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn submitted_ranges_are_not_reused_before_real_completion(){let mut p=Planner::default();p.add_page(1,64).unwrap();let a=p.reserve(32,16).unwrap().unwrap();p.publish(a).unwrap();p.mark_used(a,7).unwrap();p.retire(a).unwrap();let b=p.reserve(32,16).unwrap().unwrap();assert_eq!(b.range.offset,32);assert!(p.reserve(1,1).unwrap().is_none());assert!(p.rollback(a).is_err());assert_eq!(p.collect(6).unwrap(),0);assert!(p.reserve(1,1).unwrap().is_none());assert_eq!(p.collect(7).unwrap(),1);let c=p.reserve(32,16).unwrap().unwrap();assert_eq!(c.range,a.range);assert_ne!(c.id,a.id);assert!(p.publish(a).is_err());}
 #[test]fn failed_candidates_rollback_without_touching_published_ranges(){let mut p=Planner::default();p.add_page(1,96).unwrap();let old=p.reserve(16,16).unwrap().unwrap();p.publish(old).unwrap();p.mark_used(old,2).unwrap();let before=p.metrics();let a=p.reserve(17,32).unwrap().unwrap();assert_eq!(a.range.offset,32);let b=p.reserve(48,16).unwrap();assert!(b.is_none());p.rollback(a).unwrap();assert_eq!(p.metrics(),before);let all=p.reserve(80,16).unwrap().unwrap();assert_eq!(all.range.offset,16);assert!(p.mark_used(all,3).is_err());assert!(p.reserve(1,0).is_err());assert!(p.add_page(1,99).is_err());}
 #[test]fn packed_vertex_stride_reservations_are_exact(){let mut p=Planner::default();p.add_page(1,1024).unwrap();let a=p.reserve(96,12).unwrap().unwrap();let b=p.reserve(120,12).unwrap().unwrap();assert_eq!(a.range.offset%12,0);assert_eq!(b.range.offset%12,0);assert_eq!(b.range.offset,96);p.rollback(a).unwrap();p.rollback(b).unwrap();p.remove_empty_page(1).unwrap();assert_eq!(p.metrics().page_bytes,0);}

}

#[cfg(test)]mod capacity_tests{use super::*;
 #[test]fn large_shared_budget_pages_and_completed_reclamation(){let mut p=Planner::default();p.add_page(1,96*1024*1024).unwrap();let a=p.reserve(40*1024*1024,12).unwrap().unwrap();assert_eq!(p.empty_pages().count(),0);p.publish(a).unwrap();p.mark_used(a,8).unwrap();p.retire(a).unwrap();p.collect(7).unwrap();assert_eq!(p.empty_pages().count(),0);p.collect(8).unwrap();assert_eq!(p.empty_pages().collect::<Vec<_>>(),vec![(1,96*1024*1024)]);p.remove_empty_page(1).unwrap();}
 #[test]fn pending_fragmented_pages_never_reclaim(){let mut p=Planner::default();p.add_page(1,128).unwrap();let a=p.reserve(32,4).unwrap().unwrap();let b=p.reserve(32,4).unwrap().unwrap();p.rollback(a).unwrap();assert_eq!(p.empty_pages().count(),0);assert!(p.remove_empty_page(1).is_err());p.rollback(b).unwrap();assert_eq!(p.empty_pages().count(),1);}
}
