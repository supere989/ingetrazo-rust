//! Cached source tool label producer; native CAD retains each frozen snapshot.
use crate::annotation_overlay_geometry::{Cache, Composition, Limits};
use ingetrazo_render::{
    annotations::{Gate, Operation, Plan, Primitive},
    cad2d::Pose,
    cad_callback::{Factory, GlyphOperator},
};
use std::sync::Arc;
#[derive(PartialEq)]
struct ToolKey {
    distance: f64,
    point: [f64; 3],
    matrix: [[f32; 4]; 4],
    rect: [f32; 4],
    density: f32,
    unit: &'static str,
    precision: i32,
    typed: String,
}
#[derive(PartialEq)]
struct DocumentKey {revision:u64,scene_version:u64,view:u64,selected:Option<usize>,matrix:[[f32;4];4],rect:[f32;4],density:f32,style:serde_json::Value,units:serde_json::Value,bypass:bool,distance:Option<(f64,[f64;3])>,typed:String}
#[derive(PartialEq)]enum Key {Tool(ToolKey),Document(DocumentKey)}
pub(crate) struct Candidate {
    ready: Option<Arc<(Key, Arc<ingetrazo_metal::egui_painter::AdmittedCallback>)>>,
}
impl Candidate {
    pub fn shape(&self, rect: egui::Rect) -> Option<egui::Shape> {
        self.ready.as_ref().map(|ready| shape(rect, ready.1.clone()))
    }
}
pub(crate) struct Labels {
    cache: Cache,
    factory: Factory,
    native_builds: u64,
    uploaded_bytes: u64,
    current: Option<Arc<(Key, Arc<ingetrazo_metal::egui_painter::AdmittedCallback>)>>,
}
impl Labels {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            cache: Cache::new(
                ingetrazo_formats::annotation_font::Limits {
                    faces: 32,
                    retained_font_bytes: 32 * 1024 * 1024,
                },
                Limits {
                    cells: 2_000_000,
                    spans: 200_000,
                    geometry_bytes: 32 * 1024 * 1024,
                    glyphs: 4096,
                    glyph_bytes: 16 * 1024 * 1024,
                },
            )?,
            factory: Factory::default(),
            native_builds: 0,
            uploaded_bytes: 0,
            current: None,
        })
    }
    pub fn prepare(
        &mut self,
        app: &crate::IngeTrazoApp,
        rect: egui::Rect,
        density: f32,
        metal: Arc<ingetrazo_metal::NativeMetal>,
    ) -> Result<Candidate, String> {
        if !app.scene.dimensions.is_empty()||!app.scene.text_labels.is_empty(){return self.prepare_document(app,rect,density,metal);}
        let Some((distance, point)) = app.active_tool.distance_label() else {
            return Ok(Candidate { ready: None });
        };
        if !distance.is_finite()
            || !point.is_finite()
            || !density.is_finite()
            || density <= 0.
            || !rect.is_finite()
        {
            return Err("Metal tool label finite admission".into());
        }
        let camera = crate::navigation_ray::camera_snapshot(&app.camera)
            .ok_or("Metal label camera admission")?;
        let (unit, precision) = units(&app.scene)?;
        let key = Key::Tool(ToolKey {
            distance,
            point: point.to_array(),
            matrix: camera.matrix,
            rect: [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
            density,
            unit,
            precision,
            typed: app.vcb_text.clone(),
        });
        if let Some(ready) = &self.current {if ready.0==key{return Ok(Candidate {ready:Some(ready.clone())});}}
        let v = point.as_vec3().extend(1.).to_array();
        let q: [f32; 4] = std::array::from_fn(|row| {
            camera.matrix[0][row] * v[0]
                + camera.matrix[1][row] * v[1]
                + camera.matrix[2][row] * v[2]
                + camera.matrix[3][row] * v[3]
        });
        if !q.iter().all(|v| v.is_finite()) || q[3] <= 0. {
            return Ok(Candidate { ready: None });
        }
        let pixel = [
            f64::from(rect.min.x)
                + (f64::from(q[0]) / f64::from(q[3]) + 1.) * 0.5 * f64::from(rect.width()),
            f64::from(rect.min.y)
                + (1. - f64::from(q[1]) / f64::from(q[3])) * 0.5 * f64::from(rect.height()),
        ];
        let text: Arc<str> = if app.vcb_text.is_empty() {
            ingetrazo_render::annotations::format_length(distance, unit, precision)?
        } else {
            typed_text(&app.vcb_text, unit)
        }
        .into();
        let shadow = if app.vcb_text.is_empty() {
            [255, 255, 255, 220]
        } else {
            [255, 220, 130, 235]
        };
        let operations = [([12., -7.], shadow), ([11., -8.], [15, 20, 27, 255])]
            .into_iter()
            .map(|(offset, color)| Operation {
                primitive: Primitive::Text {
                    position: [pixel[0] + offset[0], pixel[1] + offset[1]],
                    text: text.clone(),
                    size_pt: 10,
                    color,
                },
                gate: Gate::Always,
            })
            .collect();
        let plan = Arc::new(Plan {
            queries: vec![],
            operations,
        });
        let clip = [
            (rect.min.x * density).floor() as i32,
            (rect.min.y * density).floor() as i32,
            (rect.max.x * density).ceil() as i32,
            (rect.max.y * density).ceil() as i32,
        ];
        let mesh = self
            .cache
            .prepare_widget_clipped(plan, density, 96., clip)?;
        if mesh.batches.is_empty() {
            return Ok(Candidate { ready: None });
        }
        let mut glyphs = Vec::new();
        glyphs
            .try_reserve_exact(mesh.gates.len())
            .map_err(|_| "Metal label operator admission")?;
        for gate in mesh.gates.iter() {
            if gate.gate != Gate::Always {
                return Err("Metal label unexpected depth gate".into());
            }
            let composition = match gate.composition {
                Composition::SpanByte => 0,
                Composition::GlyphRgba64Opaque => 1,
                Composition::GlyphLcd(m) => {
                    2 | (u32::from(m[0]) << 8) | (u32::from(m[1]) << 16) | (u32::from(m[2]) << 24)
                }
                _ => return Err("Metal label unsupported source operator".into()),
            };
            glyphs.push(GlyphOperator {
                composition,
                straight_rgba: gate.straight_rgba,
            });
        }
        let producer = self.factory.glyph_handle(
            mesh.geometry.clone(),
            mesh.batches.clone(),
            Pose::default(),
            glyphs.into(),
        )?;
        let handle = ingetrazo_metal::egui_painter::AdmittedCallback::prepare(
            metal,
            producer.snapshot()?,
            density,
            self.current.as_ref().map(|ready| ready.1.as_ref()),
        )?;
        self.native_builds += 1;
        self.uploaded_bytes += handle.uploaded_bytes();
        Ok(Candidate {
            ready: Some(Arc::new((key, handle))),
        })
    }
    fn prepare_document(&mut self,app:&crate::IngeTrazoApp,rect:egui::Rect,density:f32,metal:Arc<ingetrazo_metal::NativeMetal>)->Result<Candidate,String>{
        use ingetrazo_metal::annotation::{Draw,Parameters,Visibility};
        if !density.is_finite()||density<=0.||!rect.is_finite()||rect.width()<=0.||rect.height()<=0.{return Err("Metal document annotation viewport admission".into());}
        let camera=crate::navigation_ray::camera_snapshot(&app.camera).ok_or("Metal document annotation camera admission")?;
        let bypass=matches!(app.style.visual_style,ingetrazo_render::style::VisualStyle::XRay|ingetrazo_render::style::VisualStyle::Wireframe);
        // Immutable admission fastpath precedes metadata/String clones and plan work.
        if let Some(ready)=&self.current{if let Key::Document(old)=&ready.0{
            if old.revision==app.native_geometry_revision&&old.scene_version==app.scene.version&&old.view==app.scene.view_version&&old.selected==app.selected_dimension&&old.matrix==camera.matrix&&old.rect==[rect.min.x,rect.min.y,rect.max.x,rect.max.y]&&old.density==density&&old.style==app.scene.native_document["dimension_style"]&&old.units==app.scene.native_document["units"]&&old.bypass==bypass&&old.distance==app.active_tool.distance_label().map(|(d,p)|(d,p.to_array()))&&old.typed==app.vcb_text{return Ok(Candidate{ready:Some(ready.clone())});}
        }}
        let key=Key::Document(DocumentKey{revision:app.native_geometry_revision,scene_version:app.scene.version,view:app.scene.view_version,selected:app.selected_dimension,matrix:camera.matrix,rect:[rect.min.x,rect.min.y,rect.max.x,rect.max.y],density,style:app.scene.native_document["dimension_style"].clone(),units:app.scene.native_document["units"].clone(),bypass,distance:app.active_tool.distance_label().map(|(d,p)|(d,p.to_array())),typed:app.vcb_text.clone()});
        if let Some(ready)=&self.current{if ready.0==key{return Ok(Candidate{ready:Some(ready.clone())});}}
        let inverse=camera.inverse().ok_or("Metal document annotation inverse admission")?;
        let matrix=std::array::from_fn(|i|camera.matrix[i/4][i%4]);
        let plan=Arc::new(crate::annotation_plan::build_plan(app,rect,&matrix,&mut self.cache,true)?);
        if plan.operations.is_empty(){return Ok(Candidate{ready:None});}
        let extent=crate::viewport_extent::scene_extent([f64::from(rect.width()),f64::from(rect.height())],f64::from(density))?;
        let mut queries=Vec::new();queries.try_reserve_exact(plan.queries.len().max(1)).map_err(|_|"Metal annotation CPU query admission")?;
        for &point in &plan.queries{let mut q=ingetrazo_render::annotation_query::prepare_query(&matrix,point,camera.eye,camera.forward,extent)?;if bypass{q.active=0;}queries.push(q);}
        let clip=[(rect.min.x*density).floor()as i32,(rect.min.y*density).floor()as i32,(rect.max.x*density).ceil()as i32,(rect.max.y*density).ceil()as i32];
        let mesh=self.cache.prepare_widget_clipped(plan,density,96.,clip)?;
        if mesh.batches.is_empty(){return Ok(Candidate{ready:None});}
        let mut draws=Vec::new();let mut glyphs=Vec::new();draws.try_reserve_exact(mesh.gates.len()).map_err(|_|"Metal annotation draw admission")?;glyphs.try_reserve_exact(mesh.gates.len()).map_err(|_|"Metal annotation glyph admission")?;
        for draw in mesh.gates.iter(){
            let gates=match draw.gate{Gate::Always=>[u32::MAX;2],Gate::Point(a)=>[a,u32::MAX],Gate::Both(a,b)=>[a,b]};
            let mut points=[0.;16];let (kind,composition)=match draw.composition{
                Composition::SpanByte=>(0,0),Composition::GlyphRgba64Opaque=>(0,1),Composition::GlyphLcd(m)=>(0,2|(u32::from(m[0])<<8)|(u32::from(m[1])<<16)|(u32::from(m[2])<<24)),
                Composition::WidgetTriangle(bits)=>{for(i,b)in bits.into_iter().enumerate(){points[i]=f32::from_bits(b)*density;}(1,0)},
                _=>return Err("Metal document annotation source operator unsupported".into())
            };
            draws.push(Draw{gates,kind,padding:0,points});glyphs.push(GlyphOperator{composition,straight_rgba:draw.straight_rgba});
        }
        let (sample,_)=ingetrazo_metal::annotation::source_sample_policy(&metal,4)?;
        let params=Parameters{inverse:std::array::from_fn(|i|inverse[i/4][i%4]),eye:[camera.eye[0],camera.eye[1],camera.eye[2],0.],forward:[camera.forward[0],camera.forward[1],camera.forward[2],0.],extent_count_sample:[extent[0],extent[1],queries.len()as u32,sample]};
        let visibility=Visibility::prepare(&metal,&queries,&draws,params,self.current.as_ref().and_then(|ready|ready.1.annotation_visibility()))?;
        let producer=self.factory.glyph_handle(mesh.geometry.clone(),mesh.batches.clone(),Pose::default(),glyphs.into())?;
        let handle=ingetrazo_metal::egui_painter::AdmittedCallback::prepare_annotation(metal,producer.snapshot()?,density,self.current.as_ref().map(|ready|ready.1.as_ref()),Some(visibility))?;
        self.native_builds+=1;self.uploaded_bytes+=handle.uploaded_bytes();Ok(Candidate{ready:Some(Arc::new((key,handle)))})
    }
    /// Cold engineering control: replace only the font preparation cache with a
    /// real too-small font admission. Accepted native owners stay untouched.
    pub fn probe_font_budget(&mut self, refuse: bool) -> Result<(), String> {
        self.cache = Cache::new(
            ingetrazo_formats::annotation_font::Limits {
                faces: 32,
                retained_font_bytes: if refuse { 1 } else { 32 * 1024 * 1024 },
            },
            Limits {
                cells: 2_000_000,
                spans: 200_000,
                geometry_bytes: 32 * 1024 * 1024,
                glyphs: 4096,
                glyph_bytes: 16 * 1024 * 1024,
            },
        )?;
        Ok(())
    }
    pub fn stats(&self) -> (u64, u64) {
        (self.native_builds, self.uploaded_bytes)
    }
    pub fn admitted(&self) -> Option<Arc<ingetrazo_metal::egui_painter::AdmittedCallback>> {
        self.current.as_ref().map(|ready| ready.1.clone())
    }
    pub fn publish(&mut self, candidate: Candidate) {
        self.current = candidate.ready;
    }
}
fn shape(
    rect: egui::Rect,
    handle: Arc<ingetrazo_metal::egui_painter::AdmittedCallback>,
) -> egui::Shape {
    egui::Shape::Callback(egui::PaintCallback {
        rect,
        callback: handle.token(),
    })
}
fn units(scene: &ingetrazo_core::scene::Scene) -> Result<(&'static str, i32), String> {
    use ingetrazo_core::units::LengthUnit;
    let fallback = match scene.unit {
        LengthUnit::Meters => "m",
        LengthUnit::Centimeters => "cm",
        LengthUnit::Millimeters => "mm",
        LengthUnit::Inches => "in",
        LengthUnit::Feet => "ft",
    };
    let unit = match scene.native_document["units"]["length"]
        .as_str()
        .unwrap_or(fallback)
    {
        "m" => "m",
        "cm" => "cm",
        "mm" => "mm",
        "in" => "in",
        "ft" => "ft",
        "ft-in" => "ft-in",
        "in-frac" => "in-frac",
        "ft-in-frac" => "ft-in-frac",
        _ => return Err("Unknown tool label unit".into()),
    };
    Ok((
        unit,
        i32::try_from(
            scene.native_document["units"]["precision"]
                .as_i64()
                .unwrap_or(2),
        )
        .map_err(|_| "Tool precision overflow")?,
    ))
}
fn typed_text(buffer: &str, unit: &str) -> String {
    let text = buffer.trim();
    let bare = !text.is_empty()
        && text.split([',', ';']).all(|part| {
            let p = part.trim();
            let p = p.strip_prefix(['-', '+']).unwrap_or(p);
            !p.is_empty() && p.chars().all(|c| c.is_ascii_digit() || c == '.')
        });
    if !bare {
        return text.into();
    }
    let mark = match unit {
        "m" => " m",
        "cm" => " cm",
        "mm" => " mm",
        "ft" => "'",
        _ => "\"",
    };
    format!("{text}{mark}")
}
