//! Direct cached position/UV submission. Replacement descriptors and images are
//! prepared together; old descriptors are destroyed before fence-retired images.
use super::{resources::{BufferId,Resources},texture_cache::{AssetId,PreparedTexture,TextureCache},VulkanContext};
use ash::{vk,Device};
use ingetrazo_render::{buffer::TextureBuffers,native_textures::Geometry};
use std::collections::{HashMap,HashSet};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum Phase{Opaque,BackOpaque,TransparentFront,TransparentBack,Mask,Depth,Billboard}
#[derive(Clone,Debug)]struct Draw{buffer:BufferId,set:vk::DescriptorSet,asset:AssetId,first:u32,count:u32,opacity:f32,shade:f32,cutout:bool,back:bool,cull:bool,opaque:bool,path:String,order:usize,mean:Option<[f32;3]>,shadow_only:bool,billboard:bool,missing:bool,bitmap:bool}
struct Pool{device:Device,handle:vk::DescriptorPool}
impl Drop for Pool{fn drop(&mut self){unsafe{self.device.destroy_descriptor_pool(self.handle,None)}}}
pub(crate) struct Publication{retirement:super::resources::CompletedRetirement,active:HashSet<AssetId>}
pub struct Candidate{pool:Option<Pool>,buffers:Vec<BufferId>,draws:Vec<Draw>,images:Vec<PreparedTexture>,references:Option<super::reference_images::Candidate>,pub contours:Vec<(BufferId,u32,u32)>,averages:super::texture_average::Candidate,average_active:HashSet<AssetId>}
#[derive(Default,Clone,Copy,Debug)]pub struct Stats{pub builds:u64,pub uploads:u64,pub vertex_bytes:u64,pub draws:u64,pub cache_hits:u64,pub rollbacks:u64,pub average_decodes:u64,pub average_hits:u64,pub average_entries:usize,pub sampled_entries:usize}
pub struct Textures{references:super::reference_images::ReferenceImages,image_key:Option<(u64,u64)>,probe_pressure:bool,budget:std::sync::Arc<super::budget::VulkanBudget>,device:Device,layout:vk::PipelineLayout,descriptor_layout:vk::DescriptorSetLayout,pipelines:[vk::Pipeline;16],billboard_pipelines:[vk::Pipeline;2],averages:super::texture_average::Cache,pool:Option<Pool>,buffers:Vec<BufferId>,draws:Vec<Draw>,cache:TextureCache,pub stats:Stats}
pub(crate) struct VertexUpdate{old:BufferId,new:BufferId,index:usize,draws:Vec<usize>,bytes:usize}
pub(crate) struct VertexRefresh{updates:Vec<VertexUpdate>,retirement:super::resources::CompletedRetirement}
impl Textures{
 pub(crate) fn asset_layout(&self)->vk::DescriptorSetLayout{self.descriptor_layout}
 pub fn new(context:&VulkanContext,render_pass:vk::RenderPass,samples:vk::SampleCountFlags,budget:std::sync::Arc<super::budget::VulkanBudget>)->Result<Self,String>{
  let device=context.device.clone();let mut result=Self{references:super::reference_images::ReferenceImages::new(context,render_pass,samples)?,image_key:None,probe_pressure:false,budget,device,layout:vk::PipelineLayout::null(),descriptor_layout:vk::DescriptorSetLayout::null(),pipelines:[vk::Pipeline::null();16],billboard_pipelines:[vk::Pipeline::null();2],averages:super::texture_average::Cache::new(ingetrazo_render::source_texture_decode::Limits::default(),65536),pool:None,buffers:vec![],draws:vec![],cache:TextureCache::default(),stats:Stats::default()};
  let bindings=[vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT)];
  result.descriptor_layout=unsafe{result.device.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),None)}.map_err(|e|e.to_string())?;
  result.layout=unsafe{result.device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&[result.descriptor_layout]).push_constant_ranges(&[vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT).offset(0).size(128)]),None)}.map_err(|e|e.to_string())?;
  let clip_distance=unsafe{context.instance.get_physical_device_features(context.physical_device)}.shader_clip_distance!=0;
  for index in 0..16{result.pipelines[index]=pipeline(&result.device,render_pass,result.layout,samples,index,context.source_depth_clip_enabled,clip_distance)?;}
  // Source disables polygon offset before its camera-facing placement pass.
  for (slot,index) in [0,8].into_iter().enumerate(){result.billboard_pipelines[slot]=pipeline_source_mode(&result.device,render_pass,result.layout,samples,index,false,true,context.source_depth_clip_enabled,clip_distance,false)?;}Ok(result)
 }
 pub fn prepare(&mut self,context:&VulkanContext,resources:&mut Resources,buffers:&TextureBuffers,assets:&HashMap<String,Vec<u8>>,scene:&ingetrazo_core::scene::Scene,fence:vk::Fence,completed:u64,storage_limit:u64)->Result<Candidate,String>{
  self.prepare_mode(context,resources,buffers,assets,scene,fence,completed,storage_limit,false)
 }
 pub fn prepare_mode(&mut self,context:&VulkanContext,resources:&mut Resources,buffers:&TextureBuffers,assets:&HashMap<String,Vec<u8>>,scene:&ingetrazo_core::scene::Scene,fence:vk::Fence,completed:u64,storage_limit:u64,average:bool)->Result<Candidate,String>{
  self.prepare_shadow_mode(context,resources,buffers,assets,scene,fence,completed,storage_limit,average,false,&[])
 }
 pub(crate) fn prepare_shadow_mode(&mut self,context:&VulkanContext,resources:&mut Resources,buffers:&TextureBuffers,assets:&HashMap<String,Vec<u8>>,scene:&ingetrazo_core::scene::Scene,fence:vk::Fence,completed:u64,storage_limit:u64,average:bool,shadow:bool,extra:&[&TextureBuffers])->Result<Candidate,String>{
  if !unsafe{context.device.get_fence_status(fence)}.map_err(|e|e.to_string())?{return Err("Texture replacement requires completed draw fence".into());}
  let mut candidate=Candidate{pool:None,buffers:vec![],draws:vec![],images:vec![],references:None,contours:vec![],averages:super::texture_average::Candidate::default(),average_active:HashSet::new()};
  let prepare=(||->Result<(),String>{
   {let _pressure=if self.probe_pressure{let mut ledger=self.budget.ledger.lock().unwrap();let previous=ledger.total_limit;ledger.total_limit=Some(ledger.total_committed());Some(PressureRestore{budget:self.budget.clone(),previous})}else{None};candidate.references=Some(self.references.prepare(context,resources,&mut self.cache,scene,fence).map_err(|e|format!("Reference image admission: {e}"))?);}
   if buffers.is_empty()&&extra.iter().all(|b|b.is_empty()){return Ok(());}
   let count=u32::try_from(buffers.len().checked_add(extra.iter().try_fold(0usize,|n,b|n.checked_add(b.len())).ok_or("Shadow texture count overflow")?).ok_or("Shadow descriptor count overflow")?).map_err(|_|"Too many texture descriptors")?;
   let mut paths=buffers.iter().map(|(p,b)|(p,b,false)).chain(extra.iter().flat_map(|b|b.iter().map(|(p,b)|(p,b,true)))).collect::<Vec<_>>();paths.sort_by_key(|(_,b,_)|b.source_texture_runs.iter().map(|r|r.order).min().unwrap_or(usize::MAX));
   let mut images=candidate.references.as_ref().unwrap().sampled().collect::<HashMap<AssetId,(vk::ImageView,vk::Sampler,bool)>>();
   let mut pending_image_bytes=0usize;let mut pending_image_start=0usize;
   for(path,buffer,extra_only)in paths{
    let billboard=buffer.source_billboard_receiver;let shadow_only=extra_only&&!billboard;
    let geometry=Geometry::prepare(buffer)?;if geometry.vertices.is_empty(){continue;}
    // viewport.py1612/1780: shaded opaque FRONTS use the Qt mean;
    // all back overrides and translucent fronts sample the original texture.
    // One asset can need both; retain one geometry upload and one sampled image.
    let needs_mean=!billboard&&!shadow_only&&average&&geometry.draws.iter().any(|d|!d.source.back_side&&d.source.opacity>=0.999);
    let needs_sampled=if billboard{buffer.source_billboard_color.is_none()}else{geometry.draws.iter().any(|d|!average||d.source.back_side||d.source.opacity<0.999||(shadow&&!d.source.back_side&&d.source.opacity>=0.7))};
    let file;let missing;
    let encoded=if buffer.source_billboard_color.is_some(){missing=false;&[][..]}else if let Some(bytes)=assets.get(path){missing=false;bytes.as_slice()}else{
     let read=if needs_mean{read_average_file(path,self.averages.limits().encoded_bytes)}else{match std::fs::read(path){Ok(bytes)=>Ok(bytes),Err(error)if (shadow||billboard)&&error.kind()==std::io::ErrorKind::NotFound=>Ok(Vec::new()),Err(error)=>Err(format!("Texture {path}: {error}"))}};
     file=read?;missing=(shadow||billboard)&&!needs_mean&&file.is_empty();file.as_slice()
    };
    let asset=self.cache.identify(encoded);let id=asset.id();
    let mean=if let Some(color)=buffer.source_billboard_color{Some(color)}else if needs_mean{Some(self.averages.prepare(&mut candidate.averages,id,encoded)?)}else{None};
    let (sampled,set)=if needs_sampled&&!missing {
     // Lazy pool creation keeps pure opaque shaded scenes free of sampled
     // image/descriptor allocations, including when changing render styles.
     if candidate.pool.is_none(){let handle=unsafe{self.device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(count).pool_sizes(&[vk::DescriptorPoolSize{ty:vk::DescriptorType::COMBINED_IMAGE_SAMPLER,descriptor_count:count}]),None)}.map_err(|e|e.to_string())?;candidate.pool=Some(Pool{device:self.device.clone(),handle});}
     let pool=candidate.pool.as_ref().unwrap().handle;
     let sampled=if let Some(sampled)=images.get(&id){*sampled}else{let _pressure=if self.probe_pressure{let mut ledger=self.budget.ledger.lock().unwrap();let previous=ledger.total_limit;ledger.total_limit=Some(ledger.total_committed());Some(PressureRestore{budget:self.budget.clone(),previous})}else{None};let image=self.cache.prepare_deferred(context,resources,&asset,256*1024*1024).map_err(|e|format!("Texture image admission: {e}"))?;let sampled=image.sampled();images.insert(id,sampled);pending_image_bytes=pending_image_bytes.checked_add(image.pending_bytes()).ok_or("Pending texture image bytes overflow")?;candidate.images.push(image);if pending_image_bytes>=8*1024*1024{self.cache.complete_many(context,resources,candidate.images[pending_image_start..].iter_mut())?;pending_image_bytes=0;pending_image_start=candidate.images.len();}sampled};
     let set=unsafe{self.device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&[self.descriptor_layout]))}.map_err(|e|e.to_string())?[0];
     let infos=[vk::DescriptorImageInfo::default().image_view(sampled.0).sampler(sampled.1).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];unsafe{self.device.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&infos)],&[])};
     (sampled,set)
    }else{((vk::ImageView::null(),vk::Sampler::null(),false),vk::DescriptorSet::null())};
    let vertex=resources.upload(context,geometry.upload_bytes(),vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER,false)?;candidate.buffers.push(vertex);self.stats.uploads+=1;self.stats.vertex_bytes+=geometry.upload_bytes().len()as u64;
    if !billboard&&!shadow_only&&geometry.front_vertices>0 {
     if geometry.upload_bytes().len()as u64<=storage_limit{candidate.contours.push((vertex,geometry.front_vertices/3,5));}
     else {let triangles=storage_limit.min(8*1024*1024)/36;if triangles==0{return Err("Texture contour storage range cannot hold one triangle".into());}for chunk in geometry.vertices[..geometry.front_vertices as usize].chunks(triangles as usize*3){let positions=chunk.iter().map(|v|v.position).collect::<Vec<_>>();let id=resources.upload(context,bytemuck::cast_slice(&positions),vk::BufferUsageFlags::STORAGE_BUFFER,false)?;candidate.buffers.push(id);candidate.contours.push((id,(positions.len()/3)as u32,3));}}
    }
    for draw in geometry.draws{let source=draw.source;let opaque=source.opacity>=0.999;let mean=if opaque&&!source.back_side{mean}else{None};let opacity=if opaque{1.}else{format!("{:.3}",source.opacity).parse::<f32>().map_err(|e|e.to_string())?};candidate.draws.push(Draw{buffer:vertex,set,asset:id,first:draw.first,count:draw.count,opacity,shade:source.shade,cutout:billboard||sampled.2,back:source.back_side,cull:source.front_cull,opaque,path:path.clone(),order:source.order,mean,shadow_only,billboard,missing,bitmap:buffer.source_shadow_bitmap});}
   }
   self.cache.complete_many(context,resources,candidate.images[pending_image_start..].iter_mut())?;
   // Source front opaque first occurrence, then sorted back/transparent keys.
   candidate.average_active.try_reserve(candidate.draws.len()).map_err(|_|"Average active identity admission")?;
   candidate.average_active.extend(candidate.draws.iter().filter(|d|d.mean.is_some()).map(|d|d.asset));
   self.averages.reserve_publication(&candidate.averages)?;
   candidate.draws.sort_by(|a,b|{let phase=|d:&Draw|if d.opaque&&!d.back{0}else if d.opaque{1}else if !d.back{2}else{3};phase(a).cmp(&phase(b)).then_with(||if phase(a)==0{a.order.cmp(&b.order)}else{a.path.cmp(&b.path).then(a.shade.total_cmp(&b.shade)).then(a.opacity.total_cmp(&b.opacity)).then(a.cull.cmp(&b.cull))})});Ok(())
  })();
  if let Err(error)=prepare{self.abort(resources,candidate,fence,completed)?;return Err(error);}self.stats.average_decodes=self.averages.stats.decodes;self.stats.average_hits=self.averages.stats.hits;self.stats.builds+=1;Ok(candidate)
 }
 pub fn abort(&mut self,resources:&mut Resources,mut candidate:Candidate,fence:vk::Fence,completed:u64)->Result<(),String>{drop(candidate.pool.take());for buffer in candidate.buffers{resources.retire(buffer)?;}resources.collect(fence,completed)?;self.stats.rollbacks+=1;Ok(())}
 pub(crate) fn preflight_publication(&mut self,context:&VulkanContext,resources:&Resources,candidate:&Candidate,fence:vk::Fence,completed:u64,extra_old:&[BufferId],extra_next:&[BufferId])->Result<Publication,String>{
  if candidate.references.is_none(){return Err("Texture candidate lacks reference publication".into());}
  let mut old=Vec::new();old.try_reserve_exact(self.buffers.len().checked_add(extra_old.len()).ok_or("Retirement count overflow")?).map_err(|_|"Retirement list admission")?;old.extend_from_slice(&self.buffers);old.extend_from_slice(extra_old);
  let mut next=Vec::new();next.try_reserve_exact(candidate.buffers.len().checked_add(extra_next.len()).ok_or("Candidate count overflow")?).map_err(|_|"Candidate list admission")?;next.extend_from_slice(&candidate.buffers);next.extend_from_slice(extra_next);
  self.cache.preflight_publication(candidate.images.iter().chain(candidate.references.iter().flat_map(|r|r.images())))?;
  let mut active=HashSet::new();let count=candidate.draws.len().checked_add(candidate.references.as_ref().map_or(0,|r|r.assets().count())).ok_or("Active texture count overflow")?;
  active.try_reserve(count).map_err(|_|"Active texture identity admission")?;
  active.extend(candidate.draws.iter().filter(|d|d.set!=vk::DescriptorSet::null()).map(|d|d.asset).chain(candidate.references.iter().flat_map(|r|r.assets())));
  let retirement=resources.publication_preflight(context,fence,completed,&old,&next)?;
  Ok(Publication{retirement,active})
 }
 pub(crate) fn publish_admitted(&mut self,resources:&mut Resources,mut candidate:Candidate,publication:Publication,completed:u64,scene:&ingetrazo_core::scene::Scene){
  for image in candidate.images.drain(..){self.cache.publish_admitted(image);}
  if let Some(references)=candidate.references.take(){self.references.publish_admitted(&mut self.cache,references,scene.image_planes.publication_revision());self.image_key=Some((scene.image_planes.membership_epoch(),scene.view_version));}
  drop(self.pool.take());resources.retire_completed(publication.retirement);
  self.pool=candidate.pool.take();self.buffers=candidate.buffers;self.draws=candidate.draws;
  self.cache.collect_unused_completed(&publication.active,completed);
  self.averages.publish(candidate.averages,&candidate.average_active);self.stats.average_entries=self.averages.stats.retained_entries;self.stats.sampled_entries=self.cache.published_entries();
 }
 pub fn publish(&mut self,context:&VulkanContext,resources:&mut Resources,candidate:Candidate,fence:vk::Fence,completed:u64,scene:&ingetrazo_core::scene::Scene)->Result<(),String>{
  let publication=match self.preflight_publication(context,resources,&candidate,fence,completed,&[],&[]){Ok(proof)=>proof,Err(error)=>{self.abort(resources,candidate,fence,completed)?;return Err(error);}};
  self.publish_admitted(resources,candidate,publication,completed,scene);Ok(())
 }

 /// No allocation, geometry traversal, hashing, decoding, or transfer on draw.
 pub fn record(&mut self,resources:&mut Resources,command:vk::CommandBuffer,serial:u64,phase:Phase,mvp:&[f32;16],plane:&[f32;4],clip:u32,height:u32,xray:bool)->Result<(),String>{
  self.record_receiver(resources,command,serial,phase,mvp,plane,clip,height,xray,None)
 }
 pub(crate) fn record_receiver(&mut self,resources:&mut Resources,command:vk::CommandBuffer,serial:u64,phase:Phase,mvp:&[f32;16],plane:&[f32;4],clip:u32,height:u32,xray:bool,shadow:Option<(vk::PipelineLayout,&[vk::Pipeline;16])>)->Result<(),String>{
  let (layout,pipelines)=shadow.unwrap_or((self.layout,&self.pipelines));
  for draw in self.draws.iter().filter(|d|!d.shadow_only&&d.billboard==matches!(phase,Phase::Billboard)&&(!d.missing||matches!(phase,Phase::Mask|Phase::Depth))).filter(|d|match phase{Phase::Opaque=>d.opaque&&!d.back,Phase::BackOpaque=>d.opaque&&d.back,Phase::TransparentFront=>!d.opaque&&!d.back,Phase::TransparentBack=>!d.opaque&&d.back,Phase::Mask|Phase::Depth|Phase::Billboard=>true}){
   let buffer=resources.buffer(draw.buffer).ok_or("Cached texture buffer retired before draw")?;resources.mark_used(draw.buffer,serial)?;if draw.set!=vk::DescriptorSet::null(){self.cache.mark_used(draw.asset,serial)?;}
   // viewport.py1652: XRay affects opaque fronts only; back and translucent
   // texture phases retain their paint opacity. _xray_face_depth uses all
   // texture positions without sampling alpha or writing colour.
   let index=if phase==Phase::Depth{7}else if phase==Phase::Mask{5}else if phase==Phase::Opaque&&xray{6}else if draw.opaque{if draw.back{1}else{0}}else if draw.back{4}else if draw.cull{3}else{2};
   let mut push=[0u8;112];push[..64].copy_from_slice(bytemuck::cast_slice(mvp));push[64..80].copy_from_slice(bytemuck::cast_slice(plane));push[80..84].copy_from_slice(bytemuck::bytes_of(&(clip|if matches!(phase,Phase::Mask|Phase::Depth){2}else{0})));push[84..88].copy_from_slice(bytemuck::bytes_of(&if phase==Phase::Opaque&&xray{0.6f32}else{draw.opacity}));push[88..92].copy_from_slice(bytemuck::bytes_of(&draw.shade));push[92..96].copy_from_slice(bytemuck::bytes_of(&(height<<1|u32::from(draw.cutout))));
   if let Some(mean)=draw.mean{push[96..108].copy_from_slice(bytemuck::cast_slice(&mean));}
   unsafe{self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,if phase==Phase::Billboard{self.billboard_pipelines[usize::from(draw.mean.is_some()||draw.missing)]}else{pipelines[index+if draw.mean.is_some()||draw.missing{8}else{0}]});if draw.mean.is_none()&&!draw.missing{self.device.cmd_bind_descriptor_sets(command,vk::PipelineBindPoint::GRAPHICS,layout,0,&[draw.set],&[]);}self.device.cmd_bind_vertex_buffers(command,0,&[buffer],&[0]);self.device.cmd_push_constants(command,layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,0,&push);self.device.cmd_draw(command,draw.count,1,draw.first,0);}self.stats.draws+=1;
  }Ok(())
 }
 /// Front-only source casters; receiver averaging never removes alpha cutout.
 pub(crate) fn preflight_shadow(&mut self,resources:&mut Resources,serial:u64,independent:bool)->Result<(),String>{for draw in self.draws.iter().filter(|d|!d.billboard&&!d.back&&d.opacity>=0.7&&(!independent||d.shadow_only)&&!(d.bitmap&&d.missing)){resources.mark_used(draw.buffer,serial)?;if !draw.missing{self.cache.mark_used(draw.asset,serial)?;}}Ok(())}
 pub(crate) fn record_shadow(&mut self,resources:&mut Resources,command:vk::CommandBuffer,serial:u64,pipeline:vk::Pipeline,layout:vk::PipelineLayout,fallback:vk::Pipeline,plain_layout:vk::PipelineLayout,parameters:&[u8],independent:bool)->Result<(),String>{
  for draw in self.draws.iter().filter(|d|!d.billboard&&!d.back&&d.opacity>=0.7&&(!independent||d.shadow_only)&&!(d.bitmap&&d.missing)){
   let buffer=resources.buffer(draw.buffer).ok_or("Shadow texture buffer retired")?;
   if draw.set==vk::DescriptorSet::null()&&!draw.missing{return Err("Shadow caster sampled alpha was not admitted".into());}
   resources.mark_used(draw.buffer,serial)?;
   let mut push=[0u8;88];push.copy_from_slice(parameters);
   let(pipeline,layout)=if draw.missing{push[84..88].copy_from_slice(&0u32.to_ne_bytes());(fallback,plain_layout)}else{self.cache.mark_used(draw.asset,serial)?;(pipeline,layout)};
   unsafe{self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,pipeline);if !draw.missing{self.device.cmd_bind_descriptor_sets(command,vk::PipelineBindPoint::GRAPHICS,layout,0,&[draw.set],&[]);}self.device.cmd_bind_vertex_buffers(command,0,&[buffer],&[0]);self.device.cmd_push_constants(command,layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,0,&push);self.device.cmd_draw(command,draw.count,1,draw.first,0);}
  }Ok(())
 }
 /// Sun changes replace only sprite vertices; sampled images/descriptors and
 /// ordinary scene buffers remain immutable and resident.
 pub(crate) fn prepare_vertex_refresh(&mut self,context:&VulkanContext,resources:&mut Resources,sources:&TextureBuffers,fence:vk::Fence,completed:u64,shadow:bool)->Result<VertexRefresh,String>{
  let mut updates:Vec<VertexUpdate>=Vec::new();updates.try_reserve(sources.len()).map_err(|_|"Sprite replacement metadata admission")?;
  let mut old=Vec::new();let mut next=Vec::new();old.try_reserve(sources.len()).map_err(|_|"Sprite old identity admission")?;next.try_reserve(sources.len()).map_err(|_|"Sprite next identity admission")?;
  let result=(||->Result<(),String>{for(path,source)in sources{
   if shadow&&!source.source_shadow_bitmap{return Err("Sprite replacement source classification".into());}
   let geometry=Geometry::prepare(source)?;let mut matching=Vec::new();matching.try_reserve_exact(geometry.draws.len()).map_err(|_|"Sprite draw identity admission")?;matching.extend(self.draws.iter().enumerate().filter(|(_,d)|(if shadow{d.shadow_only&&d.bitmap}else{d.billboard})&&d.path==*path).map(|(i,_)|i));if matching.len()!=geometry.draws.len()||matching.is_empty(){return Err("Sprite replacement run identity".into());}
   let previous=self.draws[matching[0]].buffer;
   for(&i,g)in matching.iter().zip(&geometry.draws){let d=&self.draws[i];if d.buffer!=previous||d.first!=g.first||d.count!=g.count||d.opacity!=1.||d.back||(if shadow{!d.shadow_only}else{!d.billboard}){return Err("Sprite replacement immutable layout".into());}}
   let index=self.buffers.iter().position(|id|*id==previous).ok_or("Sprite buffer ownership")?;
   let buffer=resources.upload(context,geometry.upload_bytes(),vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER,false)?;old.push(previous);next.push(buffer);updates.push(VertexUpdate{old:previous,new:buffer,index,draws:matching,bytes:geometry.upload_bytes().len()});
  }if self.draws.iter().any(|d|(if shadow{d.shadow_only&&d.bitmap}else{d.billboard})&&!sources.contains_key(&d.path)){return Err("Sprite replacement removes immutable identity".into());}Ok(())})();
  let retirement=match result.and_then(|_|resources.publication_preflight(context,fence,completed,&old,&next)){Ok(proof)=>proof,Err(error)=>{for id in next{resources.retire(id)?;}resources.collect(fence,completed)?;return Err(error);}};
  Ok(VertexRefresh{updates,retirement})
 }
 pub(crate) fn publish_vertex_refresh(&mut self,resources:&mut Resources,candidate:VertexRefresh){
  for update in candidate.updates{debug_assert_eq!(self.buffers[update.index],update.old);self.buffers[update.index]=update.new;for index in update.draws{self.draws[index].buffer=update.new;}self.stats.uploads+=1;self.stats.vertex_bytes+=update.bytes as u64;}resources.retire_completed(candidate.retirement);
 }
 pub(crate) fn abort_vertex_refresh(&mut self,resources:&mut Resources,candidate:VertexRefresh,fence:vk::Fence,completed:u64)->Result<(),String>{for update in candidate.updates{resources.retire(update.new)?;}resources.collect(fence,completed)?;Ok(())}
 pub(crate) fn refresh_shadow_bitmaps(&mut self,context:&VulkanContext,resources:&mut Resources,sources:&TextureBuffers,fence:vk::Fence,completed:u64)->Result<(),String>{let candidate=self.prepare_vertex_refresh(context,resources,sources,fence,completed,true)?;self.publish_vertex_refresh(resources,candidate);Ok(())}
 pub fn refresh_images(&mut self,context:&VulkanContext,resources:&mut Resources,scene:&ingetrazo_core::scene::Scene,fence:vk::Fence,completed:u64)->Result<(),String>{
  let key=(scene.image_planes.membership_epoch(),scene.view_version);
  if self.image_key==Some(key){return self.references.refresh_pose(scene);}
  let candidate=self.references.prepare(context,resources,&mut self.cache,scene,fence)?;
  self.cache.preflight_publication(candidate.images())?;
  let mut active=HashSet::new();active.try_reserve(self.draws.len().checked_add(candidate.assets().count()).ok_or("Image refresh identity overflow")?).map_err(|_|"Image refresh identity admission")?;
  active.extend(self.draws.iter().filter(|d|d.set!=vk::DescriptorSet::null()).map(|d|d.asset).chain(candidate.assets()));
  let retirement=resources.publication_preflight(context,fence,completed,&[],&[])?;
  self.references.publish_admitted(&mut self.cache,candidate,scene.image_planes.publication_revision());
  resources.retire_completed(retirement);self.image_key=Some(key);self.cache.collect_unused_completed(&active,completed);self.stats.sampled_entries=self.cache.published_entries();Ok(())
 }
 pub fn record_images(&mut self,command:vk::CommandBuffer,serial:u64,mvp:&[f32;16])->Result<(),String>{self.references.record(&mut self.cache,command,serial,mvp)}
 pub fn image_stats(&self)->super::reference_images::Stats{self.references.stats}
 pub fn discard_average_means(&mut self){self.averages.clear();self.stats.average_entries=0;}
 pub fn set_publication_entry_limit(&mut self,limit:Option<usize>){self.cache.set_publication_entry_limit(limit);}
 pub fn configure_average(&mut self,limits:ingetrazo_render::source_texture_decode::Limits,max_entries:usize)->Result<(),String>{self.averages.configure(limits,max_entries)}
 pub fn set_pressure_for_probe(&mut self,enabled:bool){self.probe_pressure=enabled;}
 pub fn cache_stats(&self)->&super::texture_cache::TextureStats{&self.cache.stats}
 pub fn buffers(&self)->&[BufferId]{&self.buffers}
}
impl Drop for Textures{fn drop(&mut self){drop(self.pool.take());unsafe{for p in self.pipelines.into_iter().chain(self.billboard_pipelines){self.device.destroy_pipeline(p,None);}self.device.destroy_pipeline_layout(self.layout,None);self.device.destroy_descriptor_set_layout(self.descriptor_layout,None);}}}
pub(crate) fn shadow_pipeline(device:&Device,pass:vk::RenderPass,layout:vk::PipelineLayout,samples:vk::SampleCountFlags,index:usize,source_depth:bool,clip_distance:bool,terminate:bool)->Result<vk::Pipeline,String>{pipeline_source_mode(device,pass,layout,samples,index,true,false,source_depth,clip_distance,terminate)}
fn pipeline(device:&Device,pass:vk::RenderPass,layout:vk::PipelineLayout,samples:vk::SampleCountFlags,index:usize,source_depth:bool,clip_distance:bool)->Result<vk::Pipeline,String>{
 pipeline_source_mode(device,pass,layout,samples,index,false,false,source_depth,clip_distance,false)
}
fn pipeline_source(device:&Device,pass:vk::RenderPass,layout:vk::PipelineLayout,samples:vk::SampleCountFlags,index:usize,shadow:bool)->Result<vk::Pipeline,String>{
 pipeline_source_mode(device,pass,layout,samples,index,shadow,false,false,false,false)
}
fn pipeline_source_mode(device:&Device,pass:vk::RenderPass,layout:vk::PipelineLayout,samples:vk::SampleCountFlags,index:usize,shadow:bool,billboard:bool,source_depth:bool,clip_distance:bool,terminate:bool)->Result<vk::Pipeline,String>{
 let load=|bytes:&[u8]|ash::util::read_spv(&mut std::io::Cursor::new(bytes)).map_err(|e|e.to_string());let v=load(if clip_distance{include_bytes!("shaders/texture_clip.vert.spv").as_slice()}else{include_bytes!("shaders/texture.vert.spv").as_slice()})?;let f=load(if shadow&&index%8==0{if index<8{if terminate{include_bytes!("shaders/texture_shadow_terminate.frag.spv").as_slice()}else{include_bytes!("shaders/texture_shadow.frag.spv").as_slice()}}else{include_bytes!("shaders/texture_average_shadow.frag.spv").as_slice()}}else if index<8{include_bytes!("shaders/texture.frag.spv").as_slice()}else{include_bytes!("shaders/texture_average.frag.spv").as_slice()})?;let index=index%8;
 let vertex=unsafe{device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&v),None)}.map_err(|e|e.to_string())?;
 let fragment=match unsafe{device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&f),None)}{Ok(f)=>f,Err(e)=>{unsafe{device.destroy_shader_module(vertex,None)};return Err(e.to_string());}};
 let clipped=u32::from(clip_distance);let entry=[vk::SpecializationMapEntry{constant_id:0,offset:0,size:4}];let specialization=vk::SpecializationInfo::default().map_entries(&entry).data(bytemuck::bytes_of(&clipped));
 let stages=[vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vertex).name(c"main"),vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fragment).name(c"main").specialization_info(&specialization)];
 let bindings=[vk::VertexInputBindingDescription{binding:0,stride:20,input_rate:vk::VertexInputRate::VERTEX}];let attrs=[vk::VertexInputAttributeDescription{location:0,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:0},vk::VertexInputAttributeDescription{location:1,binding:0,format:vk::Format::R32G32_SFLOAT,offset:12}];let input=vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&bindings).vertex_attribute_descriptions(&attrs);let assembly=vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);let viewport=vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
 let raster=vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).line_width(1.).front_face(vk::FrontFace::CLOCKWISE).cull_mode(if matches!(index,1|4|5){vk::CullModeFlags::FRONT}else if index==3{vk::CullModeFlags::BACK}else{vk::CullModeFlags::NONE}).depth_bias_enable(!billboard&&matches!(index,0|2|3|4|6|7)).depth_bias_constant_factor(1.).depth_bias_slope_factor(1.);
 let msaa=vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(samples);let stencil=vk::StencilOpState::default().compare_op(vk::CompareOp::ALWAYS).fail_op(vk::StencilOp::KEEP).depth_fail_op(vk::StencilOp::KEEP).pass_op(vk::StencilOp::REPLACE).compare_mask(255).write_mask(255).reference(1);let depth=vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(true).depth_write_enable(index<2||index==7).depth_compare_op(vk::CompareOp::LESS_OR_EQUAL).stencil_test_enable(index==5).front(stencil).back(stencil);
 let colors=[vk::PipelineColorBlendAttachmentState::default().blend_enable(!matches!(index,5|7)).src_color_blend_factor(vk::BlendFactor::SRC_ALPHA).dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA).color_blend_op(vk::BlendOp::ADD).src_alpha_blend_factor(vk::BlendFactor::ZERO).dst_alpha_blend_factor(vk::BlendFactor::ONE).alpha_blend_op(vk::BlendOp::ADD).color_write_mask(if matches!(index,5|7){vk::ColorComponentFlags::empty()}else{vk::ColorComponentFlags::RGBA})];let blend=vk::PipelineColorBlendStateCreateInfo::default().attachments(&colors);let mut states=vec![vk::DynamicState::VIEWPORT,vk::DynamicState::SCISSOR];if source_depth{states.push(vk::DynamicState::DEPTH_CLIP_NEGATIVE_ONE_TO_ONE_EXT);}let dynamic=vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&states);
 let result=unsafe{device.create_graphics_pipelines(vk::PipelineCache::null(),&[vk::GraphicsPipelineCreateInfo::default().stages(&stages).vertex_input_state(&input).input_assembly_state(&assembly).viewport_state(&viewport).rasterization_state(&raster).multisample_state(&msaa).depth_stencil_state(&depth).color_blend_state(&blend).dynamic_state(&dynamic).layout(layout).render_pass(pass)],None)};unsafe{device.destroy_shader_module(vertex,None);device.destroy_shader_module(fragment,None);}
 match result{Ok(p)=>Ok(p[0]),Err((partial,e))=>{for p in partial{unsafe{device.destroy_pipeline(p,None)}}Err(e.to_string())}}
}

struct PressureRestore{budget:std::sync::Arc<super::budget::VulkanBudget>,previous:Option<u64>}
impl Drop for PressureRestore{fn drop(&mut self){self.budget.ledger.lock().unwrap().total_limit=self.previous;}}


// Actual encoded read is bounded even if the file grows after metadata inspection.
fn read_average_file(path:&str,max:usize)->Result<Vec<u8>,String>{
 use std::io::Read;
 let file=std::fs::File::open(path).map_err(|e|format!("Texture {path}: {e}"))?;
 let cap=max.checked_add(1).ok_or("Average encoded admission overflow")?;
 let mut bytes=Vec::new();file.take(cap as u64).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
 if bytes.len()>max{return Err("Average encoded admission".into());}Ok(bytes)
}
