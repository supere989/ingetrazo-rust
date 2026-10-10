//! Qt document-style editing groups, bounded to one inline session.
use std::collections::VecDeque;
#[derive(Debug,Clone,PartialEq)]struct Span{len:usize,tag:u64}
#[derive(Debug,Clone,PartialEq)]pub struct Snapshot{pub text:String,pub cursor:(usize,usize),spans:Vec<Span>}
#[derive(Debug,Clone,Copy,PartialEq)]enum Kind{Insert(u64),Backspace(u64),Delete(u64),SelectionDelete,Newline}
#[derive(Debug,Clone)]struct Command{before:Snapshot,after:Snapshot,kind:Kind}
#[derive(Debug,Clone)]pub struct History{current:Snapshot,undo:VecDeque<Command>,redo:Vec<Command>,break_group:bool,next_tag:u64}
fn slice(spans:&[Span],start:usize,end:usize)->Vec<Span>{let mut out=vec![];let mut at=0;for s in spans{let lo=start.max(at);let hi=end.min(at+s.len);if hi>lo{out.push(Span{len:hi-lo,tag:s.tag});}at+=s.len;}out}
fn replace(spans:&[Span],at:usize,removed:usize,len:usize,tag:u64)->Vec<Span>{let total=spans.iter().map(|s|s.len).sum();let mut out=slice(spans,0,at);if len>0{out.push(Span{len,tag});}out.extend(slice(spans,at+removed,total));let mut merged:Vec<Span>=vec![];for s in out{if let Some(last)=merged.last_mut().filter(|p|p.tag==s.tag){last.len+=s.len;}else{merged.push(s);}}merged}
impl History{
 pub fn new(text:&str)->Self{let n=text.chars().count();Self{current:Snapshot{text:text.into(),cursor:(n,n),spans:if n>0{vec![Span{len:n,tag:0}]}else{vec![]}},undo:VecDeque::new(),redo:vec![],break_group:false,next_tag:1}}
 pub fn cursor(&self)->(usize,usize){self.current.cursor}
 pub fn undo(&mut self)->Option<Snapshot>{let cmd=self.undo.pop_back()?;self.current=cmd.before.clone();self.redo.push(cmd);self.break_group=true;Some(self.current.clone())}
 pub fn redo(&mut self)->Option<Snapshot>{let cmd=self.redo.pop()?;self.current=cmd.after.clone();self.undo.push_back(cmd);self.break_group=false;Some(self.current.clone())}
 fn tag(&mut self)->u64{let tag=self.next_tag;self.next_tag=self.next_tag.checked_add(1).expect("inline edit identity overflow");tag}
 fn insert_tag(&mut self)->u64{if !self.break_group{if let Some(c)=self.undo.back(){if c.after.cursor==self.current.cursor{if let Kind::Insert(tag)=c.kind{return tag;}}}}self.tag()}
 pub fn observe(&mut self,text:&str,cursor:(usize,usize)){
  if self.current.text==text{if self.current.cursor!=cursor{self.break_group=true;}self.current.cursor=cursor;return;}
  let before=self.current.clone();let old:Vec<char>=before.text.chars().collect();let new:Vec<char>=text.chars().collect();let mut prefix=old.iter().zip(&new).take_while(|(a,b)|a==b).count();let mut suffix=old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(a,b)|a==b).count();
  // Repeated characters make longest-prefix diffs ambiguous. Use the
  // actual collapsed caret to identify the edit if that reconstruction fits.
  if cursor.0==cursor.1{let selected=before.cursor.0.abs_diff(before.cursor.1);if let Some(inserted)=new.len().checked_add(selected).and_then(|n|n.checked_sub(old.len())){if let Some(at)=cursor.0.checked_sub(inserted){let end=at+selected;if end<=old.len()&&at+inserted<=new.len()&&old[..at]==new[..at]&&old[end..]==new[at+inserted..]{prefix=at;suffix=old.len()-end;}}}else if new.len()<old.len(){let at=cursor.0;let removed=old.len()-new.len();if at+removed<=old.len()&&at<=new.len()&&old[..at]==new[..at]&&old[at+removed..]==new[at..]{prefix=at;suffix=old.len()-at-removed;}}}
  let removed=old.len()-prefix-suffix;let inserted=&new[prefix..new.len()-suffix];if !inserted.is_empty()&&prefix!=before.cursor.0.min(before.cursor.1){self.break_group=true;}
  if inserted.is_empty(){
   let backwards=cursor.0<before.cursor.0;let selection=before.cursor.0!=before.cursor.1;let mut chars=old;let mut spans=before.spans;let mut remaining=removed;
   while remaining>0{let runs=slice(&spans,prefix,prefix+remaining);let run=if selection{Span{len:remaining,tag:0}}else if backwards{runs.last().unwrap().clone()}else{runs.first().unwrap().clone()};let at=if backwards{prefix+remaining-run.len}else{prefix};chars.drain(at..at+run.len);spans=replace(&spans,at,run.len,0,0);let kind=if selection{Kind::SelectionDelete}else if backwards{Kind::Backspace(run.tag)}else{Kind::Delete(run.tag)};self.push(chars.iter().collect(),(at,at),kind,spans.clone());remaining-=run.len;}
   self.current.cursor=cursor;
  }else{
   let mut chars=old;chars.drain(prefix..prefix+removed);let mut spans=replace(&before.spans,prefix,removed,0,0);let(mut at,mut start)=(prefix,0);
   for(i,c)in inserted.iter().enumerate(){if *c=='\n'{if i>start{let tag=self.insert_tag();chars.splice(at..at,inserted[start..i].iter().copied());spans=replace(&spans,at,0,i-start,tag);at+=i-start;self.push(chars.iter().collect(),(at,at),Kind::Insert(tag),spans.clone());}let tag=self.tag();chars.insert(at,'\n');spans=replace(&spans,at,0,1,tag);at+=1;self.push(chars.iter().collect(),(at,at),Kind::Newline,spans.clone());start=i+1;}}
   if start<inserted.len(){let tag=self.insert_tag();chars.splice(at..at,inserted[start..].iter().copied());spans=replace(&spans,at,0,inserted.len()-start,tag);self.push(chars.iter().collect(),cursor,Kind::Insert(tag),spans);}else{self.current.cursor=cursor;}
  }
 }
 fn push(&mut self,text:String,cursor:(usize,usize),kind:Kind,spans:Vec<Span>){let after=Snapshot{text,cursor,spans};let merge=!self.break_group&&kind!=Kind::Newline&&kind!=Kind::SelectionDelete&&self.undo.back().is_some_and(|c|c.kind==kind&&c.after.cursor==self.current.cursor);if merge{self.undo.back_mut().unwrap().after=after.clone();}else{self.undo.push_back(Command{before:self.current.clone(),after:after.clone(),kind});if self.undo.len()>100{self.undo.pop_front();}}self.current=after;self.redo.clear();self.break_group=false;}
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn frozen_qt_typing_newline_and_cursor_groups(){let mut h=History::new("Text");h.observe("Texthello",(9,9));h.observe("Texthello\nworld",(15,15));assert_eq!(h.undo().unwrap().text,"Texthello\n");assert_eq!(h.undo().unwrap().text,"Texthello");assert_eq!(h.undo().unwrap().text,"Text");assert_eq!(h.redo().unwrap().text,"Texthello");assert_eq!(h.redo().unwrap().text,"Texthello\n");assert_eq!(h.redo().unwrap().text,"Texthello\nworld");let mut h=History::new("Text");h.observe("Textabcd",(8,8));h.observe("Textabcd",(6,6));h.observe("TextabXcd",(7,7));assert_eq!(h.undo().unwrap().text,"Textabcd");assert_eq!(h.undo().unwrap().text,"Text");}
 #[test]fn frozen_qt_replacement_and_backspace_groups(){let mut h=History::new("Text");h.observe("Text",(4,0));h.observe("r",(1,1));h.observe("replacement",(11,11));assert_eq!(h.undo().unwrap().text,"Text");let mut h=History::new("Text");h.observe("Textabcd",(8,8));h.observe("Textabc",(7,7));h.observe("Textab",(6,6));assert_eq!(h.undo().unwrap().text,"Textabcd");assert_eq!(h.undo().unwrap().text,"Text");}
}

#[cfg(test)]mod corpus{use super::*;
 #[test]fn frozen_source_keyboard_recipes(){let data:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/composer-text-authoring/source-local-undo-controls.json")).unwrap();for row in data["rows"].as_array().unwrap(){let mut h=History::new("Text");let mut chars:Vec<char>="Text".chars().collect();let(mut a,mut b)=(4,4);for op in row["recipe"].as_array().unwrap(){let op=op.as_str().unwrap();match op{"Undo"=>{let t=h.undo().unwrap();chars=t.text.chars().collect();a=t.cursor.0;b=t.cursor.1;},"Redo"=>{let t=h.redo().unwrap();chars=t.text.chars().collect();a=t.cursor.0;b=t.cursor.1;},"Control+a"=>{a=chars.len();b=0;},"Left"=>{a=a.min(b).saturating_sub(1);b=a;},"Backspace"=>{if a==b&&a>0{chars.remove(a-1);a-=1;b=a;}},_=>{let value=if op=="Return"{"\n"}else{op};for c in value.chars(){let lo=a.min(b);let hi=a.max(b);chars.splice(lo..hi,[c]);a=lo+1;b=a;h.observe(&chars.iter().collect::<String>(),(a,b));}}}h.observe(&chars.iter().collect::<String>(),(a,b));}assert_eq!(chars.iter().collect::<String>(),row["final"].as_str().unwrap(),"{} final",row["name"]);let mut undo=vec![];while let Some(s)=h.undo(){undo.push(s.text);}let mut redo=vec![];while let Some(s)=h.redo(){redo.push(s.text);}assert_eq!(serde_json::json!(undo),row["undo"],"{} undo",row["name"]);assert_eq!(serde_json::json!(redo),row["redo"],"{} redo",row["name"]);}}
}
