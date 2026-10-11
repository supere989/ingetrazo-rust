//! Optional bounded UI-pass timing. Results are read only after the existing
//! graphics fence; no extra wait, CPU pixel readback or production warm allocation.
use ash::{vk,Device};
use std::sync::Arc;
use super::{VulkanContext,lifetime::Lifetime};
pub(super) struct Timing{device:Device,lifetime:Arc<Lifetime>,pub pool:vk::QueryPool,period:f64,bits:u32,samples:[f64;4096],count:usize,seen:usize,pending:bool}
impl Timing{
 pub fn new(context:&VulkanContext)->Result<Option<Self>,String>{
  if std::env::var_os("INGETRAZO_ANNOTATION_TIMING").is_none(){return Ok(None);}
  context.lifetime.check()?;
  let(bits,period)=context.graphics_timestamp_info().ok_or("Graphics timestamp capability unavailable")?;
  let pool=unsafe{context.device.create_query_pool(&vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::TIMESTAMP).query_count(2),None)}.map_err(|e|e.to_string())?;
  Ok(Some(Self{device:context.device.clone(),lifetime:context.lifetime.clone(),pool,period:f64::from(period),bits,samples:[0.;4096],count:0,seen:0,pending:false}))
 }
 pub fn complete(&mut self)->Result<(),String>{
  if !self.pending{return Ok(());}self.lifetime.check()?;
  let mut values=[[0u64;2];2];
  let result=unsafe{self.device.get_query_pool_results(self.pool,0,&mut values,vk::QueryResultFlags::TYPE_64|vk::QueryResultFlags::WITH_AVAILABILITY)};
  self.pending=false;
  match result{Ok(())=>{},Err(vk::Result::NOT_READY)=>return Ok(()),Err(e)=>return Err(e.to_string())}
  if values.iter().any(|v|v[1]==0){return Ok(());}
  let mask=if self.bits==64{u64::MAX}else{(1u64<<self.bits)-1};
  let elapsed=(values[1][0].wrapping_sub(values[0][0])&mask)as f64*self.period;
  self.seen+=1;if self.seen>100&&self.count<self.samples.len(){self.samples[self.count]=elapsed;self.count+=1;}Ok(())
 }
 pub fn submitted(&mut self){self.pending=true;}
 pub fn summary(&self)->serde_json::Value{
  let mut values=self.samples;values[..self.count].sort_by(f64::total_cmp);
  serde_json::json!({"scope":"entire existing UI renderpass draw workload, excludes World pass; GPU timestamps after existing fence","samples":self.count,"discarded_warmup":self.seen.min(100),"median_ns":if self.count==0{0.}else{values[self.count/2]},"p95_ns":if self.count==0{0.}else{values[(self.count-1)*95/100]},"timestamp_valid_bits":self.bits,"timestamp_period_ns":self.period})
 }
}
impl Drop for Timing{fn drop(&mut self){eprintln!("Native annotation GPU timing: {}",self.summary());if self.lifetime.uncertain(){self.lifetime.retain((self.device.clone(),self.pool));return;}unsafe{self.device.destroy_query_pool(self.pool,None)}}}
