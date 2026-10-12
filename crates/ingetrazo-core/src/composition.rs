// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! 2D Sheet Composer & Drawing Layout System (Track 4).
//!
//! Provides paper layout, exact drawing scales (1:50, 1:100, etc.),
//! model projection view frames (MarcoVista), title blocks (Cajetín),
//! text annotations, north arrows, and graphic scale bars.
//!
//! Model units are METERS; sheet canvas units are MILLIMETERS of paper.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// ISO 216 and standard paper sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperSize {
    A4,
    A3,
    A2,
    A1,
    A0,
    Letter,
    Legal,
}

impl Default for PaperSize {
    fn default() -> Self {
        PaperSize::A3
    }
}

impl PaperSize {
    /// Return portrait (width, height) in millimeters.
    pub fn portrait_dimensions_mm(&self) -> (f64, f64) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A2 => (420.0, 594.0),
            PaperSize::A1 => (594.0, 841.0),
            PaperSize::A0 => (841.0, 1189.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
        }
    }

    pub fn dimensions_mm(&self, orientation: Orientation) -> (f64, f64) {
        let (w, h) = self.portrait_dimensions_mm();
        match orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Landscape,
    Portrait,
}

impl Default for Orientation {
    fn default() -> Self {
        Orientation::Landscape
    }
}

/// Standard scale denominators for 1:N drawings.
pub const COMMON_SCALES: &[f64] = &[
    0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0,
];

pub const RENDER_DPI: f64 = 300.0;
pub const PT_TO_MM: f64 = 25.4 / 72.0;

/// Format scale value to architectural string: "1:100", "1:50", or "10:1".
pub fn format_scale(n: f64) -> String {
    if n <= 0.0 {
        return String::new();
    }
    if n < 1.0 {
        let val_str = format!("{:.4}", 1.0 / n);
        let trimmed = val_str.trim_end_matches('0').trim_end_matches('.');
        format!("{trimmed}:1")
    } else {
        let val_str = format!("{:.4}", n);
        let trimmed = val_str.trim_end_matches('0').trim_end_matches('.');
        format!("1:{trimmed}")
    }
}

/// Parse scale from typed string (e.g., "1:100", "50", "10:1").
pub fn parse_scale(text: &str, default: f64) -> f64 {
    let s = text.trim().replace(',', ".");
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(a), Ok(b)) = (parts[0].trim().parse::<f64>(), parts[1].trim().parse::<f64>()) {
                if a > 0.0 && b > 0.0 {
                    return b / a;
                }
            }
        }
        return default;
    }
    s.parse::<f64>().unwrap_or(default).max(0.001)
}

/// Computes the vertical span of the 3D model (in meters) visible inside a view frame of height `frame_h_mm` at scale `1:N`.
pub fn model_height_for_frame(frame_h_mm: f64, scale_n: f64) -> f64 {
    (frame_h_mm * scale_n) / 1000.0
}

/// Invert OrbitCamera's ortho half-height equation `half = distance * tan(fov / 2)` to solve for distance.
pub fn ortho_distance_for_height(model_h_m: f64, fov_deg: f64) -> f64 {
    let half = model_h_m / 2.0;
    let t = (fov_deg.to_radians() / 2.0).tan();
    if t <= 0.0 {
        return 10.0;
    }
    half / t
}

/// 2D rectangle in paper millimeters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectMm {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl RectMm {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// Model-View Frame (MarcoVista): projects 3D model onto the sheet at exact scale.
fn default_cut_fill()->String{"solid".into()}
fn default_cut_color()->String{"#595e69".into()}
fn default_cut_hatch()->f64{1.5}
fn default_cut_pen()->f64{0.5}
fn default_profile_pen()->f64{0.35}
fn default_edge_pen()->f64{0.18}
fn default_profiles()->bool{true}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewFrame {
    pub id: Uuid,
    pub rect: RectMm,
    pub scale_n: f64,
    pub view_key: String, // "__current__", "std:top", "std:front", "std:right", "std:iso", or scene name
    pub style: String,    // "sombreado", "technical", "wireframe"
    pub show_title: bool,
    pub title_text: String,
    pub border: bool,
    pub perspective: bool,
    #[serde(default="default_cut_pen")] pub pen_cut_mm:f64,
    #[serde(default="default_profile_pen")] pub pen_profile_mm:f64,
    #[serde(default="default_edge_pen")] pub pen_edge_mm:f64,
    #[serde(default="default_profiles")] pub profiles:bool,
    #[serde(default)] pub paper_bg:bool,
    #[serde(default)] pub hidden_lines:bool,
    #[serde(default="default_cut_fill")] pub cut_fill:String,
    #[serde(default="default_cut_color")] pub cut_fill_color:String,
    #[serde(default="default_cut_hatch")] pub cut_hatch_mm:f64,
    #[serde(default)] pub cam_target:Option<[f64;3]>,
    #[serde(default)] pub cam_yaw:Option<f64>,
    #[serde(default)] pub cam_pitch:Option<f64>,
    #[serde(default)] pub cam_distance:Option<f64>,
    #[serde(default)] pub cam_fov:Option<f64>,
    pub rot_deg: f64,
    pub annotations: bool,
    pub section_marks: bool,
}

impl Default for ViewFrame {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            rect: RectMm::new(20.0, 20.0, 200.0, 150.0),
            scale_n: 100.0, // 1:100
            view_key: "__current__".to_string(),
            style: "sombreado".to_string(),
            show_title: false,
            title_text: String::new(),
            border: false,
            cut_fill:default_cut_fill(),cut_fill_color:default_cut_color(),cut_hatch_mm:1.5,
            pen_cut_mm:0.5,pen_profile_mm:0.35,pen_edge_mm:0.18,profiles:true,hidden_lines:false,paper_bg:false,
            perspective: false,cam_target:None,cam_yaw:None,cam_pitch:None,cam_distance:None,cam_fov:None,
            rot_deg: 0.0,
            annotations: true,
            section_marks: true,
        }
    }
}

/// Standard Architectural Title Block (Cajetín / Rótulo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleBlock {
    pub id: Uuid,
    pub rect: RectMm,
    pub project_name: String,
    pub drawing_title: String,
    pub author: String,
    pub date: String,
    pub sheet_number: String,
    pub scale_text: String,
}

impl Default for TitleBlock {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            rect: RectMm::new(260.0, 210.0, 140.0, 65.0),
            project_name: "Active-iQ Project".to_string(),
            drawing_title: "General Layout".to_string(),
            author: "IngeTrazo CAD".to_string(),
            date: "2026-10-07".to_string(),
            sheet_number: "A-01".to_string(),
            scale_text: "1:100".to_string(),
        }
    }
}

/// Text annotation on the sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextItem {
    pub id: Uuid,
    pub pos: (f64, f64),
    pub text: String,
    pub font_size_pt: f64,
    pub bold: bool,
}

/// North Arrow graphic element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NorthArrowItem {
    pub id: Uuid,
    pub pos: (f64, f64),
    pub size_mm: f64,
    pub rot_deg: f64,
}

/// Graphic Scale Bar element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphicScaleItem {
    pub id: Uuid,
    pub pos: (f64, f64),
    pub scale_n: f64,
    pub length_mm: f64,
    pub divisions: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeItem { pub id:Uuid, pub shape:crate::sheet_shape::SheetShape }

/// Polymorphic item on the sheet page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SheetItem {
    ViewFrame(ViewFrame),
    TitleBlock(TitleBlock),
    Text(TextItem),
    NorthArrow(NorthArrowItem),
    GraphicScale(GraphicScaleItem),
    Shape(ShapeItem),
}

impl SheetItem {
    pub fn id(&self) -> Uuid {
        match self {
            Self::ViewFrame(item)=>item.id, Self::TitleBlock(item)=>item.id,
            Self::Text(item)=>item.id, Self::NorthArrow(item)=>item.id,
            Self::GraphicScale(item)=>item.id, Self::Shape(item)=>item.id,
        }
    }

    pub fn rect(&self) -> RectMm {
        match self {
            SheetItem::Shape(f) => RectMm::new(f.shape.x_mm,f.shape.y_mm,f.shape.w_mm,f.shape.h_mm),
            SheetItem::ViewFrame(f) => f.rect,
            SheetItem::TitleBlock(b) => b.rect,
            SheetItem::Text(t) => RectMm::new(t.pos.0, t.pos.1, 40.0, 10.0),
            SheetItem::NorthArrow(n) => RectMm::new(n.pos.0, n.pos.1, n.size_mm, n.size_mm),
            SheetItem::GraphicScale(s) => RectMm::new(s.pos.0, s.pos.1, s.length_mm, 8.0),
        }
    }

    pub fn set_pos(&mut self, x: f64, y: f64) {
        match self {
            SheetItem::Shape(f) => {f.shape.x_mm=x;f.shape.y_mm=y;},
            SheetItem::ViewFrame(f) => {
                f.rect.x = x;
                f.rect.y = y;
            }
            SheetItem::TitleBlock(b) => {
                b.rect.x = x;
                b.rect.y = y;
            }
            SheetItem::Text(t) => {
                t.pos = (x, y);
            }
            SheetItem::NorthArrow(n) => {
                n.pos = (x, y);
            }
            SheetItem::GraphicScale(s) => {
                s.pos = (x, y);
            }
        }
    }
}

/// Original composer border properties remain live while the border is disabled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourcePaperBorder {
    pub on: bool,
    pub mm: f64,
    pub color: String,
    pub radius_mm: f64,
    pub style: String,
}
impl Default for SourcePaperBorder {
    fn default() -> Self { Self { on:false, mm:0.5, color:"#1e242c".into(), radius_mm:0., style:"single".into() } }
}
impl SourcePaperBorder {
    /// Original from_dict ignores dormant archived properties.
    pub fn from_native(value: &serde_json::Value) -> Self {
        let mut out=Self::default();
        if value.get("on").and_then(serde_json::Value::as_bool).unwrap_or(false) {
            out.on=true;
            out.mm=value.get("mm").and_then(serde_json::Value::as_f64).unwrap_or(out.mm);
            out.radius_mm=value.get("radius_mm").and_then(serde_json::Value::as_f64).unwrap_or(out.radius_mm);
            if let Some(v)=value.get("color").and_then(serde_json::Value::as_str) {out.color=v.into();}
            if let Some(v)=value.get("style").and_then(serde_json::Value::as_str) {out.style=v.into();}
        }
        out
    }
}

/// Master Sheet container (Composición).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    #[serde(default)]
    pub source_border: Option<SourcePaperBorder>,
    #[serde(default)]
    pub source_border_baseline: Option<SourcePaperBorder>,
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
    #[serde(default)]
    pub native_items: std::collections::HashMap<Uuid, serde_json::Value>,
    #[serde(default)]
    pub native_item_baselines: std::collections::HashMap<Uuid, serde_json::Value>,
    pub id: Uuid,
    pub name: String,
    pub paper_size: PaperSize,
    pub orientation: Orientation,
    pub margin_mm: f64,
    pub items: Vec<SheetItem>,
    pub selected_item: Option<usize>,
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new("Sheet 1", PaperSize::A3, Orientation::Landscape)
    }
}

/// Cold physical export references, preserving original all_items category order.
/// Raw legends stay in the immutable portable document; no broad editing enum change.
#[derive(Clone,Copy)]
pub enum PhysicalExportItem<'a> {Typed{index:usize,item:&'a SheetItem},Legend{index:usize,value:&'a serde_json::Value}}
impl Sheet {
    pub fn physical_export_items(&self)->Result<impl Iterator<Item=PhysicalExportItem<'_>>,String>{
        const LIMIT:usize=100_000;
        let legends=match self.native_document.get("leyendas"){None|Some(serde_json::Value::Null)=>&[][..],Some(v)=>v.as_array().map(Vec::as_slice).ok_or("Invalid physical legend inventory")?};
        let count=self.items.len().checked_add(legends.len()).filter(|n|*n<=LIMIT).ok_or("Physical item inventory exceeds budget")?;
        let mut ordered=Vec::new();ordered.try_reserve_exact(count).map_err(|_|"Physical item order allocation refused")?;
        for index in self.visible_item_indices(){
            let item=&self.items[index];let metadata=self.native_items.get(&item.id());
            let z=if let SheetItem::Shape(v)=item{v.shape.z}else{metadata.and_then(|v|v.get("z")).and_then(|v|v.as_f64()).unwrap_or(0.)};
            if !z.is_finite(){return Err("Invalid physical stacking order".into());}
            let category=match item{SheetItem::ViewFrame(_)=>0,SheetItem::Text(_)=>1,SheetItem::GraphicScale(_)=>3,SheetItem::NorthArrow(_)=>4,SheetItem::Shape(_)=>6,SheetItem::TitleBlock(_)=>14};
            ordered.push((z,category,index,PhysicalExportItem::Typed{index,item}));
        }
        for(index,value)in legends.iter().enumerate(){
            let hidden=match value.get("hidden"){None=>false,Some(v)=>v.as_bool().ok_or("Invalid legend hidden flag")?};if hidden{continue;}
            let z=match value.get("z"){None=>0.,Some(v)=>v.as_f64().ok_or("Invalid legend stacking order")?};if !z.is_finite(){return Err("Invalid legend stacking order".into());}
            ordered.push((z,5,index,PhysicalExportItem::Legend{index,value}));
        }
        ordered.sort_unstable_by(|a,b|a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then_with(||a.1.cmp(&b.1)).then_with(||a.2.cmp(&b.2)));
        Ok(ordered.into_iter().map(|v|v.3))
    }

    pub fn effective_source_border(&self) -> SourcePaperBorder {
        self.source_border.clone().unwrap_or_else(|| SourcePaperBorder::from_native(&self.native_document["border"]))
    }
    pub fn set_source_border(&mut self, border: SourcePaperBorder) -> Result<(), &'static str> {
        if !border.mm.is_finite() || !border.radius_mm.is_finite() || border.mm < 0. || border.radius_mm < 0. {return Err("Invalid paper border dimensions");}
        if !self.native_document.is_object() {self.native_document=serde_json::json!({});}
        if !self.native_document["border"].is_object() {self.native_document["border"]=serde_json::json!({});}
        let value=serde_json::to_value(&border).map_err(|_|"Invalid paper border")?;
        for (key,value) in value.as_object().unwrap() {self.native_document["border"][key]=value.clone();}
        self.source_border=Some(border);
        Ok(())
    }

    /// Original composer uses stable z ordering and excludes hidden items.
    /// Retained native metadata is authoritative for these unmodeled properties.
    /// Source canvas order, including hidden items (they still supply drag
    /// snap targets). Painting and picking use the visible-only wrapper.
    pub fn stacked_item_indices(&self) -> Vec<usize> {
        self.item_indices(true)
    }
    pub fn visible_item_indices(&self) -> Vec<usize> {
        self.item_indices(false)
    }
    fn item_indices(&self, include_hidden: bool) -> Vec<usize> {
        let metadata=|index:usize|self.native_items.get(&self.items[index].id());
        let mut indices=(0..self.items.len()).filter(|&index|
            include_hidden || match &self.items[index] {SheetItem::Shape(f)=>!f.shape.hidden,_=>!metadata(index).and_then(|value|value.get("hidden")).and_then(|value|value.as_bool()).unwrap_or(false)}
        ).collect::<Vec<_>>();
        let z=|index:usize|if let SheetItem::Shape(f)=&self.items[index] {f.shape.z} else {metadata(index).and_then(|value|value.get("z")).and_then(|value|value.as_f64()).unwrap_or(0.)};
        let category=|index:usize|match &self.items[index] {SheetItem::ViewFrame(_)=>0,SheetItem::Text(_)=>1,SheetItem::GraphicScale(_)=>3,SheetItem::NorthArrow(_)=>4,SheetItem::TitleBlock(_)=>14,SheetItem::Shape(_)=>6};
        indices.sort_by(|&a,&b|z(a).partial_cmp(&z(b)).unwrap_or(std::cmp::Ordering::Equal).then_with(||category(a).cmp(&category(b))));
        indices
    }

    pub fn new(name: impl Into<String>, paper_size: PaperSize, orientation: Orientation) -> Self {
        let mut sheet = Self {
            source_border: Some(SourcePaperBorder::default()),
            source_border_baseline: None,
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            native_items: std::collections::HashMap::new(),
            native_item_baselines: std::collections::HashMap::new(),
            id: Uuid::new_v4(),
            name: name.into(),
            paper_size,
            orientation,
            margin_mm: 10.0,
            items: Vec::new(),
            selected_item: None,
        };

        // Add a default ViewFrame and TitleBlock
        let (pw, ph) = sheet.dimensions_mm();
        let frame_w = pw - 2.0 * sheet.margin_mm - 20.0;
        let frame_h = ph - 2.0 * sheet.margin_mm - 70.0;
        sheet.items.push(SheetItem::ViewFrame(ViewFrame {
            id: Uuid::new_v4(),
            rect: RectMm::new(sheet.margin_mm + 10.0, sheet.margin_mm + 10.0, frame_w, frame_h),
            scale_n: 100.0,
            view_key: "std:top".to_string(),
            style: "technical".to_string(),
            show_title: true,
            title_text: "Top Plan (1:100)".to_string(),
            border: true,
            cut_fill:default_cut_fill(),cut_fill_color:default_cut_color(),cut_hatch_mm:1.5,
            pen_cut_mm:0.5,pen_profile_mm:0.35,pen_edge_mm:0.18,profiles:true,hidden_lines:false,paper_bg:false,
            perspective: false,cam_target:None,cam_yaw:None,cam_pitch:None,cam_distance:None,cam_fov:None,
            rot_deg: 0.0,
            annotations: true,
            section_marks: true,
        }));

        let tb_w = 140.0;
        let tb_h = 50.0;
        sheet.items.push(SheetItem::TitleBlock(TitleBlock {
            id: Uuid::new_v4(),
            rect: RectMm::new(pw - sheet.margin_mm - tb_w, ph - sheet.margin_mm - tb_h, tb_w, tb_h),
            project_name: "Active-iQ Architecture".to_string(),
            drawing_title: "General Plan".to_string(),
            author: "IngeTrazo Cad Engine".to_string(),
            date: "2026-10-07".to_string(),
            sheet_number: "A-01".to_string(),
            scale_text: "1:100".to_string(),
        }));

        sheet
    }

    pub fn dimensions_mm(&self) -> (f64, f64) {
        self.paper_size.dimensions_mm(self.orientation)
    }

    pub fn printable_rect_mm(&self) -> RectMm {
        let (w, h) = self.dimensions_mm();
        RectMm::new(
            self.margin_mm,
            self.margin_mm,
            w - 2.0 * self.margin_mm,
            h - 2.0 * self.margin_mm,
        )
    }

    pub fn add_item(&mut self, item: SheetItem) {
        self.items.push(item);
        self.selected_item = Some(self.items.len() - 1);
    }

    pub fn remove_item(&mut self, idx: usize) -> Option<SheetItem> {
        if idx < self.items.len() {
            let removed = self.items.remove(idx);
            self.selected_item = None;
            Some(removed)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_dimensions() {
        let a4_p = PaperSize::A4.dimensions_mm(Orientation::Portrait);
        assert_eq!(a4_p, (210.0, 297.0));

        let a4_l = PaperSize::A4.dimensions_mm(Orientation::Landscape);
        assert_eq!(a4_l, (297.0, 210.0));

        let a3_l = PaperSize::A3.dimensions_mm(Orientation::Landscape);
        assert_eq!(a3_l, (420.0, 297.0));
    }

    #[test]
    fn test_scale_formatting_and_parsing() {
        assert_eq!(format_scale(100.0), "1:100");
        assert_eq!(format_scale(50.0), "1:50");
        assert_eq!(format_scale(0.1), "10:1");
        assert_eq!(format_scale(1.0), "1:1");

        assert_eq!(parse_scale("1:100", 50.0), 100.0);
        assert_eq!(parse_scale("1:50", 100.0), 50.0);
        assert_eq!(parse_scale("10:1", 100.0), 0.1);
        assert_eq!(parse_scale("200", 100.0), 200.0);
    }

    #[test]
    fn test_sacred_equation_cube_at_scale() {
        // A 1m cube at 1:100 on paper must measure exactly 10.00 mm.
        // model_height_for_frame(10 mm, 100) = 10 * 100 / 1000 = 1.0 m.
        let model_h = model_height_for_frame(10.0, 100.0);
        assert!((model_h - 1.0).abs() < 1e-9);

        // Inversely, for a 200mm tall frame at 1:100:
        // 200 * 100 / 1000 = 20 meters of model.
        let span = model_height_for_frame(200.0, 100.0);
        assert!((span - 20.0).abs() < 1e-9);
    }

    #[test]
    fn test_sheet_creation_and_bounds() {
        let sheet = Sheet::new("Plano A3", PaperSize::A3, Orientation::Landscape);
        let (w, h) = sheet.dimensions_mm();
        assert_eq!((w, h), (420.0, 297.0));
        assert_eq!(sheet.items.len(), 2);

        let print_area = sheet.printable_rect_mm();
        assert_eq!(print_area.x, 10.0);
        assert_eq!(print_area.y, 10.0);
        assert_eq!(print_area.w, 400.0);
        assert_eq!(print_area.h, 277.0);
    }
}

#[cfg(test)] mod native_paint_order_tests {
 use super::*;
 #[test] fn equal_z_uses_original_all_items_category_order(){
  let mut sheet=Sheet::new("Order",PaperSize::A4,Orientation::Portrait);sheet.items.clear();
  sheet.items.push(SheetItem::NorthArrow(NorthArrowItem{id:Uuid::new_v4(),pos:(0.,0.),size_mm:10.,rot_deg:0.}));
  sheet.items.push(SheetItem::GraphicScale(GraphicScaleItem{id:Uuid::new_v4(),pos:(0.,0.),scale_n:100.,length_mm:20.,divisions:4}));
  sheet.items.push(SheetItem::ViewFrame(ViewFrame::default()));
  assert_eq!(sheet.visible_item_indices(),vec![2,1,0]);
 }
 #[test] fn hidden_items_are_excluded_and_equal_z_keeps_document_order(){
  let mut sheet=Sheet::new("Order",PaperSize::A4,Orientation::Portrait);sheet.items.clear();
  for z in [2.,-1.,2.,0.] {
   let item=SheetItem::ViewFrame(ViewFrame::default());
   sheet.native_items.insert(item.id(),serde_json::json!({"z":z,"hidden":false}));sheet.items.push(item);
  }
  sheet.native_items.get_mut(&sheet.items[3].id()).unwrap()["hidden"]=serde_json::json!(true);
  let before=sheet.clone();assert_eq!(sheet.visible_item_indices(),vec![1,0,2]);assert_eq!(sheet,before);
 }
}

#[cfg(test)]
mod source_paper_border_tests {
    use super::*;
    #[test]
    fn disabled_runtime_properties_survive_recovery_and_refusal_is_atomic() {
        let mut sheet=Sheet::default();
        let mut border=sheet.effective_source_border();
        border.color="#ffffff".into();border.radius_mm=3.;
        sheet.set_source_border(border.clone()).unwrap();
        let recovered:Sheet=serde_json::from_slice(&serde_json::to_vec(&sheet).unwrap()).unwrap();
        assert_eq!(recovered.effective_source_border(),border);
        let before=sheet.clone();border.mm=f64::NAN;
        assert!(sheet.set_source_border(border).is_err());assert_eq!(sheet,before);
        assert_eq!(SourcePaperBorder::from_native(&serde_json::json!({"on":false,"color":"#ffffff"})),SourcePaperBorder::default());
    }
}
