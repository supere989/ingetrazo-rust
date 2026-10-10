use glam::DVec3 as V;use ingetrazo_core::{scene::{Scene,SelectionItem},image_plane::ImagePlane,math::Ray};use ingetrazo_tools::{rotate_tool::RotateTool,base::{Tool,ToolContext}};use std::sync::atomic::{AtomicU64,Ordering};
struct CountAllocator;static ALLOCS:AtomicU64=AtomicU64::new(0);
unsafe impl std::alloc::GlobalAlloc for CountAllocator{unsafe fn alloc(&self,l:std::alloc::Layout)->*mut u8{ALLOCS.fetch_add(1,Ordering::Relaxed);unsafe{std::alloc::System.alloc(l)}}unsafe fn dealloc(&self,p:*mut u8,l:std::alloc::Layout){unsafe{std::alloc::System.dealloc(p,l)}}unsafe fn realloc(&self,p:*mut u8,l:std::alloc::Layout,n:usize)->*mut u8{ALLOCS.fetch_add(1,Ordering::Relaxed);unsafe{std::alloc::System.realloc(p,l,n)}}}
#[global_allocator]static ALLOCATOR:CountAllocator=CountAllocator;
fn ctx(p:V)->ToolContext { ToolContext::new(p,Ray::new(V::new(1.,1.,10.),-V::Z)) }
fn main(){
 let mut s=Scene::new();for i in 0..50_000 { s.image_planes.push(ImagePlane::new("reference",V::X*i as f64,V::X,V::Y)); }
 let id=s.image_planes[0].id;s.select(SelectionItem::ImagePlane(0));let mut t=RotateTool::new();
 t.on_click(&mut s,&ctx(V::ZERO));t.on_click(&mut s,&ctx(V::X));t.on_move(&mut s,&ctx(V::Y));
 let scans=t.image_membership_scanned;let rebuilds=t.image_membership_rebuilds;let allocs=ALLOCS.load(Ordering::Relaxed);let start=std::time::Instant::now();
 for _ in 0..1000 {t.on_control_state(&mut s,false);}
 let repeat_allocs=ALLOCS.load(Ordering::Relaxed)-allocs;assert_eq!(repeat_allocs,0);assert_eq!(t.image_membership_scanned,scans);
 let allocs=ALLOCS.load(Ordering::Relaxed);
 for i in 0..1000 {t.on_control_state(&mut s,true);t.on_control_state(&mut s,true);t.on_control_state(&mut s,false);t.on_move(&mut s,&ctx(V::new(1.,i as f64+1.,0.)));}
 let varied_allocs=ALLOCS.load(Ordering::Relaxed)-allocs;assert_eq!(varied_allocs,0);assert_eq!(t.image_membership_scanned,scans);assert_eq!(t.image_membership_rebuilds,rebuilds);
 s.image_planes.reverse();t.on_control_state(&mut s,true);assert_eq!(t.image_membership_rebuilds,rebuilds+1);assert_eq!(s.image_planes[49_999].id,id);
 let allocs=ALLOCS.load(Ordering::Relaxed);let scans=t.image_membership_scanned;for _ in 0..1000{t.on_control_state(&mut s,true);}assert_eq!(ALLOCS.load(Ordering::Relaxed)-allocs,0);assert_eq!(t.image_membership_scanned,scans);
 t.on_control_state(&mut s,false);t.on_control_state(&mut s,true);assert!(!t.is_copy_mode);assert_eq!(s.image_planes[49_999].id,id);
 println!("{}",serde_json::json!({"planes":50000,"unchanged_noncopy_frames":1000,"varied_ctrl_cycles":1000,"unchanged_copy_frames":1000,"warm_scans":0,"warm_allocations":0,"structural_rebuilds":1,"elapsed_us":start.elapsed().as_micros(),"exact":true}));
}
