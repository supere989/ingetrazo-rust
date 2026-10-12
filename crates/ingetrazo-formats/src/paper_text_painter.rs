// SPDX-License-Identifier: GPL-3.0-or-later
//! Physical source paper text painter consuming bounded, reusable native paragraph resources.
use crate::{
    paper_font_resolver::{Request, Resolver},
    paper_text_layout::{Alignment, ParagraphCache},
    sheet_pdf::{Draw, PathOp, TextDecoration, TextDraw, TextGlyph},
};
pub struct TextStyle<'a> {
    pub text: &'a str,
    pub origin_mm: [f64; 2],
    pub width_mm: f64,
    pub size_pt: f64,
    pub family: &'a str,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub alignment: Alignment,
    pub color: [f32; 4],
}
#[derive(Default)]
pub struct Painter {
    resolver: Option<Resolver>,
    cache: ParagraphCache,
}
impl Painter {
    // qt_format_text(Qt6.12): line top=ceil(previous bottom+primary leading),
    // final height remains QFixed ascent+descent; natural width adds negative right bearing.
    fn box_metrics(&mut self, style:&TextStyle<'_>)->Result<([f64;2],Vec<(f64,f64,f64,f64)>,[f64;2]),String>{
        if !style.size_pt.is_finite()||style.size_pt<=0.||style.size_pt>10000.||!style.width_mm.is_finite()||style.width_mm<=0.||style.width_mm>1_000_000.{return Err("Invalid paper box measurement".into());}
        if self.resolver.is_none(){self.resolver=Some(Resolver::new()?);}
        let request=Request{family:style.family.into(),bold:style.bold,italic:style.italic,required_chars:vec![]};
        let resolver=self.resolver.as_mut().unwrap();let primary=resolver.resolve(request.clone())?.font;
        let scale=style.size_pt*25.4/72./100.*0.75;
        let paragraph=self.cache.layout(resolver,&request,style.text,style.width_mm/scale,style.alignment)?;
        let leading=primary.leading_26_6()as f64/64.;let mut height=-leading;let mut width=0_f64;let mut baselines=Vec::new();
        for line in &paragraph.lines {
            height=(height+leading).ceil();let baseline=height+line.line.ascent_26_6 as f64/64.;
            height+=(line.line.ascent_26_6 as f64+line.line.descent_26_6 as f64)/64.;
            let bearing=line.line.runs.last().and_then(|r|r.glyphs.last().map(|g|(r,g))).map(|(r,g)|r.font.right_bearing_26_6(g.id)).transpose()?.unwrap_or(0).min(0);
            width=width.max((line.line.advance_26_6-i64::from(bearing))as f64/64.);
            baselines.try_reserve(line.line.runs.len()).map_err(|_|"Paper box metrics allocation refused")?;for _ in &line.line.runs{baselines.push((line.baseline_26_6 as f64/64.,baseline,line.x_26_6 as f64/64.,line.line.advance_26_6 as f64/64.));}
        }
        if paragraph.lines.is_empty(){height=(primary.ascent_26_6()+primary.descent_26_6())as f64/64.;}
        Ok(([width*scale,height*scale],baselines,[width,height]))
    }
    /// QFontMetricsF.horizontalAdvance for physical source labels, without wrapping or ink bearings.
    pub fn measure_advance(&mut self, style:&TextStyle<'_>)->Result<f64,String>{
        if !style.size_pt.is_finite()||style.size_pt<=0.||style.size_pt>10000.{return Err("Invalid paper advance measurement".into());}
        if self.resolver.is_none(){self.resolver=Some(Resolver::new()?);}
        let request=Request{family:style.family.into(),bold:style.bold,italic:style.italic,required_chars:vec![]};
        let scale=style.size_pt*25.4/72./100.*0.75;
        // QFontMetricsF uses QTextEngine::width over the whole string, not paragraph max width.
        // Line/paragraph separators break shaping but contribute zero; default QTextOption tab stops are80px.
        let text=style.text.split('\u{009c}').next().unwrap_or("");
        if text.len()>crate::paper_text_layout::MAX_LINE_BYTES{return Err("Paper advance input exceeds byte budget".into());}
        let mut advance=0_i64;let mut begin=0;
        let resolver=self.resolver.as_mut().unwrap();
        for(at,ch)in text.char_indices().chain(std::iter::once((text.len(),'\0'))){
            if matches!(ch,'\n'|'\r'|'\u{2028}'|'\u{2029}'|'\t'|'\0'){
                let line=crate::paper_text_layout::shape_line(resolver,&request,&text[begin..at])?;
                advance=advance.checked_add(line.advance_26_6).ok_or("Paper advance overflow")?;
                if ch=='\t'{advance=advance.checked_div(80*64).and_then(|n|n.checked_add(1)).and_then(|n|n.checked_mul(80*64)).ok_or("Paper tab advance overflow")?;}
                begin=at+ch.len_utf8();
            }
        }
        Ok(advance as f64/64.*scale)
    }
    /// Primary 100px QFontMetricsF quantities scaled to source paper millimetres:
    /// ascent, descent, leading, cap height. Reuses the existing resolver owner.
    pub fn font_metrics_mm(&mut self,style:&TextStyle<'_>)->Result<[f64;4],String>{
        if !style.size_pt.is_finite()||style.size_pt<=0.||style.size_pt>10000.{return Err("Invalid paper font metric size".into());}
        if self.resolver.is_none(){self.resolver=Some(Resolver::new()?);}
        let request=Request{family:style.family.into(),bold:style.bold,italic:style.italic,required_chars:vec![]};
        let font=self.resolver.as_mut().unwrap().resolve(request)?.font;
        let scale=style.size_pt*25.4/72./100.*0.75;
        Ok([font.ascent_26_6(),font.descent_26_6(),font.leading_26_6(),font.cap_height_26_6()].map(|v|f64::from(v)/64.*scale))
    }
    pub fn measure_box(&mut self,style:&TextStyle<'_>)->Result<[f64;2],String>{self.box_metrics(style).map(|v|v.0)}
    /// Finite source drawText rectangle; vertical alignment is 0=top,1=center,2=bottom.
    pub fn draw_box(&mut self,style:&TextStyle<'_>,height:f64,vertical:u8)->Result<Vec<Draw>,String>{
        if !height.is_finite()||height<=0.||height>1_000_000.||vertical>2{return Err("Invalid paper text rectangle".into());}
        let(need,baselines,_)=self.box_metrics(style)?;let offset=match vertical{1=>(height-need[1])*0.5,2=>height-need[1],_=>0.};
        let mut draws=self.draw(style)?;
        if draws.len()!=baselines.len(){return Err("Paper box run inventory mismatch".into());}
        for(d,(old,new,old_x,advance))in draws.iter_mut().zip(baselines){
            if let Some(t)=&mut d.text {
                let align=match style.alignment{Alignment::Left=>0.,Alignment::Center=>(style.width_mm/t.scale_mm-advance)*0.5,Alignment::Right=>style.width_mm/t.scale_mm-advance};
                let fixed=|v:f64|((v*64.)as i64)as f64/64.;
                let dx=fixed(style.origin_mm[0]/t.scale_mm+align)-old_x;
                let dy=fixed(style.origin_mm[1]/t.scale_mm+offset/t.scale_mm)+new-old;
                for glyph in &mut t.glyphs{glyph.position_px[0]+=dx;glyph.position_px[1]+=dy;}
                for decoration in &mut t.decorations{decoration.line_px[0]+=dx;decoration.line_px[2]+=dx;decoration.line_px[1]+=dy;decoration.line_px[3]+=dy;}
            }
            d.origin_mm=[0.;2];
            // qt_format_text clips only when measured bounds exceed the rectangle.
            let[x,y]=style.origin_mm;
            d.clip=if need[0]>style.width_mm||need[1]>height{Some(vec![PathOp::Move([x,y]),PathOp::Line([x+style.width_mm,y]),PathOp::Line([x+style.width_mm,y+height]),PathOp::Line([x,y+height]),PathOp::Close])}else{None};
        }Ok(draws)
    }
    /// Qt qt_format_text performs QRectF::contains in the unscaled 100px
    /// layout coordinates. Testing scaled dimensions alone loses the rounding
    /// at a baseline rectangle's bottom edge and can omit an actual PDF clip.
    pub fn source_box_clip(&mut self, style: &TextStyle<'_>, height: f64, vertical: u8)
        -> Result<Option<Vec<PathOp>>, String>
    {
        if !height.is_finite()||height<=0.||height>1_000_000.||vertical>2||style.origin_mm.iter().any(|v|!v.is_finite()){return Err("Invalid paper text rectangle".into());}
        let (_, _, [width, text_height]) = self.box_metrics(style)?;
        let scale = style.size_pt * 25.4 / 72. / 100. * 0.75;
        let [x,y] = style.origin_mm;
        let (rx,ry,rw,rh) = (x/scale,y/scale,style.width_mm/scale,height/scale);
        let dx = match style.alignment { Alignment::Left=>0., Alignment::Center=>(rw-width)/2., Alignment::Right=>rw-width };
        let dy = match vertical { 1=>(rh-text_height)/2., 2=>rh-text_height, _=>0. };
        let contained = rx+dx>=rx && ry+dy>=ry
            && rx+dx+width<=rx+rw && ry+dy+text_height<=ry+rh;
        Ok((!contained).then(||vec![PathOp::Move([x,y]),PathOp::Line([x+style.width_mm,y]),
            PathOp::Line([x+style.width_mm,y+height]),PathOp::Line([x,y+height]),PathOp::Close]))
    }
    /// Refresh is explicit; existing pages retain their immutable font resources.
    pub fn refresh_fonts(&mut self) -> Result<(), String> {
        if let Some(resolver) = &mut self.resolver {
            resolver.refresh()?;
        }
        self.cache.clear();
        Ok(())
    }
    pub fn stats(
        &self,
    ) -> (
        (usize, usize, usize, usize, u64),
        Option<(usize, u64, (usize, usize, u64))>,
    ) {
        (
            self.cache.stats(),
            self.resolver.as_ref().map(Resolver::stats),
        )
    }
    /// Source background metrics intentionally ignore italic/underline and use primary font spacing.
    pub fn draw_with_background(
        &mut self,
        style: &TextStyle<'_>,
        color: [f32; 4],
    ) -> Result<Vec<Draw>, String> {
        if color
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("Invalid paper text background".into());
        }
        let mut draws = self.draw(style)?;
        let size_mm = style.size_pt * 25.4 / 72.;
        let scale = size_mm / 100. * 0.75;
        let (y0, y1) = if style.text.is_empty() {
            (0., size_mm)
        } else {
            let request = Request {
                family: style.family.into(),
                bold: style.bold,
                italic: false,
                required_chars: vec![],
            };
            let resolver = self.resolver.as_mut().unwrap();
            let font = resolver.resolve(request.clone())?.font;
            let paragraph = self.cache.layout(
                resolver,
                &request,
                style.text,
                style.width_mm.max(0.5) / scale,
                Alignment::Left,
            )?;
            let ascent = font.ascent_26_6() as f64 / 64.;
            let descent = font.descent_26_6() as f64 / 64.;
            let height = ascent + descent;
            let spacing = height + font.leading_26_6() as f64 / 64.;
            let n = (paragraph.height_26_6 as f64 / 64. / height.max(1e-6))
                .round_ties_even()
                .max(1.);
            (
                (ascent - font.cap_height_26_6() as f64 / 64.) * scale - 0.25 * size_mm,
                (height + (n - 1.) * spacing) * scale + 0.05 * size_mm,
            )
        };
        let h = (y1 - y0).max(1.);
        let x0 = -0.5;
        let x1 = style.width_mm + 0.5;
        draws
            .try_reserve(1)
            .map_err(|_| "Paper background draw allocation failed")?;
        draws.insert(
            0,
            Draw {picture:None, rotation_deg:0.,
                origin_mm: style.origin_mm,
                path: vec![
                    PathOp::Move([x0, y0]),
                    PathOp::Line([x1, y0]),
                    PathOp::Line([x1, y0 + h]),
                    PathOp::Line([x0, y0 + h]),
                    PathOp::Close,
                ],
                fill: Some(color),
                stroke: None,
                stroke_options: None,
                clip: None,
                clip_intersection: None,
                text: None,
                flatten_transform: false,
                even_odd: false,
            },
        );
        Ok(draws)
    }
    pub fn draw(&mut self, style: &TextStyle<'_>) -> Result<Vec<Draw>, String> {
        if style.family.len() > 1024
            || style.family.as_bytes().contains(&0)
            || style.text.len() > crate::paper_text_layout::MAX_LINE_BYTES
            || !style.size_pt.is_finite()
            || style.size_pt <= 0.
            || style.size_pt > 10000.
            || !style.width_mm.is_finite()
            || style.width_mm <= 0.
            || style.width_mm > 1_000_000.
            || style
                .origin_mm
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
            || style
                .color
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("Invalid physical paper text style".into());
        }
        if style.text.is_empty() {
            return Ok(vec![]);
        }
        if self.resolver.is_none() {
            self.resolver = Some(Resolver::new()?);
        }
        let request = Request {
            family: style.family.into(),
            bold: style.bold,
            italic: style.italic,
            required_chars: vec![],
        };
        let size_mm = style.size_pt * 25.4 / 72.;
        let scale = size_mm / 100. * 0.75;
        let height_mm =
            size_mm * 1.35 * (style.text.bytes().filter(|b| *b == b'\n').count() + 3) as f64;
        let paragraph = self.cache.layout(
            self.resolver.as_mut().unwrap(),
            &request,
            style.text,
            style.width_mm / scale,
            style.alignment,
        )?;
        let clip =
            if paragraph.height_26_6 as f64 / 64. * scale > height_mm || paragraph.exceeds_width {
                Some(vec![
                    PathOp::Move([0., 0.]),
                    PathOp::Line([style.width_mm, 0.]),
                    PathOp::Line([style.width_mm, height_mm]),
                    PathOp::Line([0., height_mm]),
                    PathOp::Close,
                ])
            } else {
                None
            };
        let mut draws = vec![];
        let mut count = 0usize;
        for line in &paragraph.lines {
            for run in &line.line.runs {
                count = count
                    .checked_add(run.glyphs.len())
                    .ok_or("Paper text glyph budget overflow")?;
                if count > crate::paper_text::MAX_GLYPHS
                    || draws.len() >= crate::sheet_pdf::MAX_DRAWS
                {
                    return Err("Paper text draw/glyph budget exceeded".into());
                }
                let mut x = (line.x_26_6 + run.x_26_6) as f64 / 64.;
                let start = x;
                let baseline = line.baseline_26_6 as f64 / 64.;
                let mut glyphs = vec![];
                glyphs
                    .try_reserve_exact(run.glyphs.len())
                    .map_err(|_| "Paper text glyph allocation failed")?;
                for glyph in &run.glyphs {
                    glyphs.push(TextGlyph {
                        glyph: glyph.id,
                        position_px: [
                            x + glyph.offset[0] as f64 / 64.,
                            baseline + glyph.offset[1] as f64 / 64.,
                        ],
                    });
                    x += glyph.advance[0] as f64 / 64.;
                }
                // QPdf emits RTL glyphs in logical order with negative relative advances.
                if run.rtl {
                    glyphs.reverse();
                }
                let mut decorations = vec![];
                if style.underline {
                    let [t, offset] = line.underline_metrics_px;
                    decorations.push(TextDecoration {
                        line_px: [
                            start.floor(),
                            baseline + offset,
                            x.floor(),
                            baseline + offset,
                        ],
                        width_px: t,
                    });
                }
                draws
                    .try_reserve(1)
                    .map_err(|_| "Paper text draw allocation failed")?;
                draws.push(Draw {picture:None, rotation_deg:0.,
                    origin_mm: style.origin_mm,
                    text: Some(TextDraw {
                        rotation_deg: 0.,
                        font: run.font.clone(),
                        scale_mm: scale,
                        italic: style.italic,
                        bold: style.bold,
                        glyphs,
                        decorations,
                    }),
                    path: vec![],
                    stroke: None,
                    stroke_options: None,
                    fill: Some(style.color),
                    clip: clip.clone(),
                    clip_intersection: None,
                    even_odd: true,
                    flatten_transform: false,
                });
            }
        }
        Ok(draws)
    }
}

impl std::fmt::Debug for Painter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaperTextPainter")
            .field("resource_stats", &self.stats())
            .finish()
    }
}
