//! Immutable source-ordered pixel-span plan for direct Metal compute.
//! Native Qt gray/cosmetic producers emit these constant-coverage rectangles.
//! Arbitrary triangles/affine poses are explicit unsupported boundaries.
use ingetrazo_render::cad2d::{Batch, Geometry};
use std::collections::BTreeMap;
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Span {
    pub rect: [i32; 4],
    pub coverage: u32,
    pub palette: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Tile {
    pub y: i32,
    pub x: i32,
    pub first: u32,
    pub count: u32,
}
pub(crate) struct SpanPlan {
    pub spans: Vec<Span>,
    pub tiles: Vec<Tile>,
    pub references: Vec<u32>,
}
pub(crate) fn compile(geometry: &Geometry, batches: &[Batch]) -> Result<SpanPlan, String> {
    geometry.validate_batches(batches)?;
    if batches.len() > 4096 {
        return Err("Metal span batch capacity4096 exceeded".into());
    }
    let mut spans = Vec::new();
    for (palette, batch) in batches.iter().enumerate() {
        if batch.first % 6 != 0 || batch.count % 6 != 0 {
            return Err("Metal compute requires complete canonical span quads".into());
        }
        for indices in geometry.indices()
            [batch.first as usize..(batch.first + batch.count) as usize]
            .chunks_exact(6)
        {
            if indices[0] != indices[3] || indices[2] != indices[4] {
                return Err("Metal compute quad topology invalid".into());
            }
            let v = [indices[0], indices[1], indices[2], indices[5]]
                .map(|i| geometry.vertices()[i as usize]);
            let a = v[0].position;
            let b = v[2].position;
            if !(a[0] < b[0] && a[1] < b[1])
                || v[1].position != [b[0], a[1]]
                || v[3].position != [a[0], b[1]]
                || v.iter().any(|p| {
                    p.coverage != v[0].coverage
                        || p.position.iter().any(|x| x.fract() != 0. || x.abs() > 1e6)
                })
            {
                return Err(
                    "Metal compute requires integer pixel-aligned constant-coverage source spans"
                        .into(),
                );
            }
            spans.push(Span {
                rect: [a[0] as i32, a[1] as i32, b[0] as i32, b[1] as i32],
                coverage: (v[0].coverage * 255.).round() as u32,
                palette: palette as u32,
            });
            if spans.len() > 100_000 {
                return Err("Metal compute span capacity100000 exceeded".into());
            }
        }
    }
    bin(spans)
}

/// Compiles the validated original rows, without recovering pixel positions from f32 geometry.
pub(crate) fn compile_source(geometry: &Geometry, batches: &[Batch]) -> Result<SpanPlan, String> {
    geometry.validate_batches(batches)?;
    if batches.len()>4096 {return Err("Metal span batch capacity4096 exceeded".into());}
    let source=geometry.source_spans().ok_or("Metal compute requires original source span metadata")?;
    let mut spans=Vec::new();
    for (palette,batch) in batches.iter().enumerate() {
        if batch.first%6!=0||batch.count%6!=0 {return Err("Metal source span batch partition invalid".into());}
        for row in &source.rows()[batch.first as usize/6..(batch.first+batch.count) as usize/6] {
            spans.push(Span{rect:[row.x,row.y,row.x+row.len,row.y+1],coverage:u32::from(row.coverage),palette:palette as u32});
            if spans.len()>100_000 {return Err("Metal compute span capacity100000 exceeded".into());}
        }
    }
    bin(spans)
}

fn bin(spans:Vec<Span>)->Result<SpanPlan,String>{
    if spans.is_empty() {
        return Err("Metal compute empty span plan refused".into());
    }
    let mut bins: BTreeMap<(i32, i32), Vec<u32>> = BTreeMap::new();
    let mut references_count = 0usize;
    for (index, span) in spans.iter().enumerate() {
        let [x0, y0, x1, y1] = span.rect;
        let xa = x0.div_euclid(8);
        let xb = (x1 - 1).div_euclid(8);
        let ya = y0.div_euclid(8);
        let yb = (y1 - 1).div_euclid(8);
        let cells = (xb as i64 - xa as i64 + 1) * (yb as i64 - ya as i64 + 1);
        if cells > 1_000_000
            || references_count
                .checked_add(cells as usize)
                .is_none_or(|n| n > 1_000_000)
        {
            return Err("Metal compute tile reference capacity1000000 exceeded".into());
        }
        references_count += cells as usize;
        for y in ya..=yb {
            for x in xa..=xb {
                bins.entry((y, x)).or_default().push(index as u32);
                if bins.len() > 65536 {
                    return Err("Metal compute sparse tile capacity65536 exceeded".into());
                }
            }
        }
    }
    let mut tiles = Vec::with_capacity(bins.len());
    let mut references = Vec::with_capacity(references_count);
    for ((y, x), ids) in bins {
        tiles.push(Tile {
            y,
            x,
            first: references.len() as u32,
            count: ids.len() as u32,
        });
        references.extend(ids);
    }
    Ok(SpanPlan {
        spans,
        tiles,
        references,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_render::cad2d::{EncodedRgba, Vertex};
    fn geometry() -> std::sync::Arc<Geometry> {
        Geometry::new(
            vec![[0., 0.], [16., 0.], [16., 16.], [0., 16.]]
                .into_iter()
                .map(|position| Vertex {
                    position,
                    coverage: 133. / 255.,
                })
                .collect(),
            vec![0, 1, 2, 0, 2, 3],
        )
        .unwrap()
    }
    #[test]
    fn ordered_overlap_not_deduplicated() {
        let g = geometry();
        let b = Batch {
            first: 0,
            count: 6,
            color: EncodedRgba::from_bytes([1, 7, 19, 128], 1.).unwrap(),
        };
        let p = compile(&g, &[b, b]).unwrap();
        assert_eq!(p.spans.len(), 2);
        assert_eq!(p.tiles.len(), 4);
        for tile in p.tiles {
            assert_eq!(
                &p.references[tile.first as usize..(tile.first + tile.count) as usize],
                &[0, 1]
            );
        }
        assert_eq!(p.spans[0].coverage, 133);
        assert_eq!(p.spans[1].palette, 1);
    }
    #[test]
    fn malformed_fractional_and_excessive_refused() {
        let g = geometry();
        let b = Batch {
            first: 0,
            count: 3,
            color: EncodedRgba::from_bytes([0, 0, 0, 255], 1.).unwrap(),
        };
        assert!(compile(&g, &[b]).is_err());
        let shifted = Geometry::new(
            g.vertices()
                .iter()
                .map(|v| Vertex {
                    position: [v.position[0] + 0.5, v.position[1]],
                    ..*v
                })
                .collect(),
            g.indices().to_vec(),
        )
        .unwrap();
        assert!(compile(&shifted, &[Batch { count: 6, ..b }]).is_err());
        let huge = Geometry::new(
            vec![[0., 0.], [1e6, 0.], [1e6, 1e6], [0., 1e6]]
                .into_iter()
                .map(|position| Vertex {
                    position,
                    coverage: 1.,
                })
                .collect(),
            g.indices().to_vec(),
        )
        .unwrap();
        assert!(compile(&huge, &[Batch { count: 6, ..b }]).is_err());
    }
    #[test]
    fn sparse_plan_matches_six_actual_qpainter_whole_images() {
        use ingetrazo_render::cad2d::EncodedRgba;
        let mut vertices = Vec::new();
        for (a, b) in [(8., 56.), (16., 48.)] {
            for position in [[a, a], [b, a], [b, b], [a, b]] {
                vertices.push(Vertex {
                    position,
                    coverage: 1.,
                });
            }
        }
        let g = Geometry::new(vertices, vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]).unwrap();
        let sources: [(u8, &[u8]); 6] = [
            (
                0,
                include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-0.rgba"),
            ),
            (
                1,
                include_bytes!("../../../docs/audits/metal-cad2d-offscreen/original/alpha-1.rgba"),
            ),
            (
                127,
                include_bytes!(
                    "../../../docs/audits/metal-cad2d-offscreen/original/alpha-127.rgba"
                ),
            ),
            (
                128,
                include_bytes!(
                    "../../../docs/audits/metal-cad2d-offscreen/original/alpha-128.rgba"
                ),
            ),
            (
                254,
                include_bytes!(
                    "../../../docs/audits/metal-cad2d-offscreen/original/alpha-254.rgba"
                ),
            ),
            (
                255,
                include_bytes!(
                    "../../../docs/audits/metal-cad2d-offscreen/original/alpha-255.rgba"
                ),
            ),
        ];
        let byte_mul = |v: u32, a: u32| {
            let t = v * a;
            (t + (t >> 8) + 128) >> 8
        };
        for (alpha, source) in sources {
            let batches = [
                Batch {
                    first: 0,
                    count: 6,
                    color: EncodedRgba::from_bytes([37, 93, 157, alpha], 1.).unwrap(),
                },
                Batch {
                    first: 6,
                    count: 6,
                    color: EncodedRgba::from_bytes([1, 7, 19, 128], 1.).unwrap(),
                },
            ];
            let plan = compile(&g, &batches).unwrap();
            let palette: Vec<[u32; 4]> = batches
                .iter()
                .map(|b| {
                    b.color
                        .qt_premultiplied8()
                        .unwrap()
                        .map(|c| (c * 255.).round() as u32)
                })
                .collect();
            let mut output = Vec::new();
            for y in 0..64i32 {
                for x in 0..64i32 {
                    let mut dst = [255u32; 4];
                    if let Some(tile) = plan
                        .tiles
                        .iter()
                        .find(|t| t.x == x.div_euclid(8) && t.y == y.div_euclid(8))
                    {
                        for id in &plan.references
                            [tile.first as usize..(tile.first + tile.count) as usize]
                        {
                            let s = plan.spans[*id as usize];
                            if x >= s.rect[0] && x < s.rect[2] && y >= s.rect[1] && y < s.rect[3] {
                                let src =
                                    palette[s.palette as usize].map(|v| byte_mul(v, s.coverage));
                                dst = std::array::from_fn(|i| {
                                    src[i] + byte_mul(dst[i], 255 - src[3])
                                });
                            }
                        }
                    }
                    output.extend(dst.map(|v| v as u8));
                }
            }
            assert_eq!(output, source, "actual QPainter alpha={alpha}");
        }
    }
}

#[cfg(test)] mod preserved_source_rows {
 use super::*;
 use ingetrazo_render::cad2d::{SourceSpan,Vertex,EncodedRgba};
 #[test] fn fractional_logical_vertices_preserve_exact_source_pixels_and_order(){
  let rows=vec![SourceSpan{x:2,y:3,len:4,coverage:127},SourceSpan{x:3,y:3,len:2,coverage:43}];
  let phase=[0.125,0.375];let density=1.5f32*1.25;
  let mut v=Vec::new();let mut i=Vec::new();
  for row in &rows {let base=v.len()as u32;for p in [[row.x,row.y],[row.x+row.len,row.y],[row.x+row.len,row.y+1],[row.x,row.y+1]] {v.push(Vertex{position:[((f64::from(p[0])-phase[0])/f64::from(density))as f32,((f64::from(p[1])-phase[1])/f64::from(density))as f32],coverage:f32::from(row.coverage)/255.});}i.extend([base,base+1,base+2,base,base+2,base+3]);}
  let g=Geometry::new_with_source_spans(v,i,vec![0..6,6..12],rows,1.5,1.25,phase).unwrap();
  let color=EncodedRgba::from_bytes([1,7,19,128],1.).unwrap();let b=[Batch{first:6,count:6,color},Batch{first:0,count:6,color}];
  assert!(compile(&g,&b).is_err());let plan=compile_source(&g,&b).unwrap();assert_eq!(plan.spans[0].rect,[3,3,5,4]);assert_eq!(plan.spans[0].coverage,43);assert_eq!(plan.spans[1].rect,[2,3,6,4]);assert_eq!(plan.references,[0,1]);
 }
}

#[repr(C)]#[derive(Clone,Copy,bytemuck::Pod,bytemuck::Zeroable)]
pub(crate) struct AnnotationSpan {pub span:Span,pub triangle:[i32;6],pub analytic:u32,pub padding:u32}
pub(crate) fn compile_annotation(geometry:&Geometry,batches:&[Batch],density:f32)->Result<(Vec<AnnotationSpan>,Vec<Tile>,Vec<u32>),String>{
 geometry.validate_batches(batches)?;
 let layers=geometry.annotation_layers().ok_or("Metal document annotation requires immutable mixed source metadata")?;
 if batches.len()>4096||density!=layers.ppp(){return Err("Metal annotation batch/density admission".into());}
 let mut spans=Vec::new();let mut plain=Vec::new();
 for (palette,batch)in batches.iter().enumerate(){
  if layers.overlaps_direct(batch.first,batch.count){
   if batch.color.0[3]!=1. {return Err("Metal direct annotation must be opaque".into());}
   for indices in geometry.indices()[batch.first as usize..(batch.first+batch.count)as usize].chunks_exact(3){
    let mut points=[0f32;6];for i in 0..3{let v=geometry.vertices()[indices[i]as usize];points[2*i]=v.position[0]*density;points[2*i+1]=v.position[1]*density;}
    if points.iter().any(|p|!p.is_finite()||p.abs()>1_000_000.){return Err("Metal annotation triangle arithmetic domain".into());}
    let rect=[points[0].min(points[2]).min(points[4]).floor()as i32,points[1].min(points[3]).min(points[5]).floor()as i32,points[0].max(points[2]).max(points[4]).ceil()as i32,points[1].max(points[3]).max(points[5]).ceil()as i32];
    if rect[0]>=rect[2]||rect[1]>=rect[3]{continue;}
    spans.try_reserve(1).map_err(|_|"Metal annotation triangle allocation")?;
    spans.push(AnnotationSpan{span:Span{rect,coverage:255,palette:palette as u32},triangle:points.map(|p|(p*256.).round_ties_even()as i32),analytic:1,padding:0});
    if spans.len()>100_000{return Err("Metal annotation triangle capacity100000 exceeded".into());}
   }
  }else{
   let end=batch.first+batch.count;let mut first=batch.first;
   let mut layer=layers.span_layers().partition_point(|(range,_)|range.end<=first);
   while first<end{
    let(range,rows)=layers.span_layers().get(layer).ok_or("Metal annotation source row partition missing")?;
    if range.start>first||range.end<=first{return Err("Metal annotation source row partition missing".into());}
    let stop=range.end.min(end);
    if (first-range.start)%6!=0||(stop-first)%6!=0{return Err("Metal annotation source row partition invalid".into());}
    let row_first=(first-range.start)as usize/6;let row_end=row_first+(stop-first)as usize/6;
    for row in &rows[row_first..row_end]{spans.try_reserve(1).map_err(|_|"Metal annotation span allocation")?;spans.push(AnnotationSpan{span:Span{rect:[row.x,row.y,row.x+row.len,row.y+1],coverage:u32::from(row.coverage),palette:palette as u32},triangle:[0;6],analytic:0,padding:0});}
    first=stop;layer+=1;
   }
  }
  if spans.len()>100_000{return Err("Metal annotation span capacity100000 exceeded".into());}
 }
 plain.try_reserve_exact(spans.len()).map_err(|_|"Metal annotation bins allocation")?;plain.extend(spans.iter().map(|s|s.span));let plan=bin(plain)?;Ok((spans,plan.tiles,plan.references))
}

#[cfg(test)] mod annotation_span_tests {
 use super::*;
 use ingetrazo_render::cad2d::{AnnotationLayer,SourceSpan,Vertex,EncodedRgba};
 #[test] fn literal_rows_and_direct_triangles_keep_partition_and_refuse_density(){
  let row=SourceSpan{x:7,y:11,len:3,coverage:173};let ppp=1.25;
  let mut vertices=Vec::new();for [x,y]in [[7.,11.],[10.,11.],[10.,12.],[7.,12.]]{vertices.push(Vertex{position:[x/ppp,y/ppp],coverage:173./255.});}
  vertices.extend([[20.,20.],[21.,20.],[20.,21.]].map(|position|Vertex{position,coverage:1.}));
  let g=Geometry::new_with_annotation_layers(vertices,vec![0,1,2,0,2,3,4,5,6],vec![AnnotationLayer::Spans{range:0..6,rows:vec![row]},AnnotationLayer::Opaque{range:6..9}],ppp).unwrap();
  let color=EncodedRgba::from_bytes([1,7,19,255],1.).unwrap();let batches=[Batch{first:0,count:6,color},Batch{first:6,count:3,color}];
  let (spans,_,refs)=compile_annotation(&g,&batches,ppp).unwrap();assert_eq!(spans.len(),2);assert_eq!(spans[0].span.rect,[7,11,10,12]);assert_eq!(spans[0].span.coverage,173);assert_eq!(spans[0].analytic,0);assert_eq!(spans[1].triangle,[6400,6400,6720,6400,6400,6720]);assert_eq!(spans[1].analytic,1);assert!(!refs.is_empty());assert!(compile_annotation(&g,&batches,1.).is_err());
 }
}
