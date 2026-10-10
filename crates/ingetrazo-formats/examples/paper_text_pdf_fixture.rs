#[path = "paper_subset_fixture.rs"]
mod subset;
// Engineering direct-embedding acceptance fixture. Not application text export.
use ingetrazo_formats::paper_text::{FtPolicy, Hinting, PaperFont};
use pdf_writer::{
    types::{CidFontType, FontFlags, SystemInfo},
    Name, Pdf, Rect, Ref, Str,
};
use std::{collections::BTreeSet, io::Write, sync::Arc};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if ![
            "latin",
            "accents",
            "bold",
            "italic",
            "serif",
            "mono",
            "combining",
            "kerns",
            "serif_bold",
        ]
        .contains(&name)
        {
            continue;
        }
        let call = &case["source_draw_calls"][0];
        let bytes: Arc<[u8]> = std::fs::read(call["font"]["file"].as_str().unwrap())
            .unwrap()
            .into();
        let font = PaperFont::new_with_outline_policy(
            bytes.clone(),
            0,
            Some(FtPolicy {
                hinting: Hinting::Vertical,
                force_auto_hint: false,
            }),
        )
        .unwrap();
        let run = font
            .shape_run(call["text"].as_str().unwrap(), None)
            .unwrap();
        let (program, cid_map) =
            subset::subset_program(&bytes, &run.iter().map(|g| g.id).collect::<Vec<_>>());
        let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
        let upem = face.units_per_em() as f32;
        let mut p = Pdf::new();
        p.catalog(Ref::new(1)).pages(Ref::new(2));
        p.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
        p.type0_font(Ref::new(5))
            .base_font(Name(b"NativePaperFont"))
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(Ref::new(6));
        {
            let mut cid = p.cid_font(Ref::new(6));
            cid.subtype(CidFontType::Type2)
                .base_font(Name(b"NativePaperFont"))
                .system_info(SystemInfo {
                    registry: Str(b"Adobe"),
                    ordering: Str(b"Identity"),
                    supplement: 0,
                })
                .font_descriptor(Ref::new(7))
                .cid_to_gid_map_predefined(Name(b"Identity"));
            let ids = run.iter().map(|g| g.id as u16).collect::<BTreeSet<_>>();
            let mut widths = cid.widths();
            for id in ids {
                widths.consecutive(
                    cid_map[&(id as u32)],
                    [
                        (face.glyph_hor_advance(ttf_parser::GlyphId(id)).unwrap() as f32 * 1000.
                            / upem)
                            .round(),
                    ],
                );
            }
        }
        let bbox = face.global_bounding_box();
        p.font_descriptor(Ref::new(7))
            .name(Name(b"NativePaperFont"))
            .flags(FontFlags::SYMBOLIC)
            .bbox(Rect::new(
                bbox.x_min as f32 * 1000. / upem,
                bbox.y_min as f32 * 1000. / upem,
                bbox.x_max as f32 * 1000. / upem,
                bbox.y_max as f32 * 1000. / upem,
            ))
            .italic_angle(face.italic_angle())
            .ascent(face.ascender() as f32 * 1000. / upem)
            .descent(face.descender() as f32 * 1000. / upem)
            .cap_height(face.ascender() as f32 * 1000. / upem)
            .stem_v(44.)
            .font_file2(Ref::new(8));
        p.stream(Ref::new(8), &program)
            .insert(Name(b"Length1"))
            .primitive(program.len() as i32);
        let mut content = Vec::new();
        let shear = if call["font"]["italic"].as_bool().unwrap() && !face.is_italic() {
            0.3
        } else {
            0.0
        };
        // Source default QPdfWriter A4 physical frame, logical DPI300/10mm margins.
        writeln!(content,"0.240000000 0 0 -0.240000000 10.0800000 831.919999 cm\nq\n0.437500000 0 0 0.437500000 236.220472 354.330708 cm\n0.117647059 0.141176477 0.172549024 rg\nBT\n/F 100 Tf 1 0 {} -1 0 0 Tm",shear).unwrap();
        let mut advance = 0i64;
        let mut previous = [0f64, 0f64];
        for g in run {
            let y = (-font.ascent_26_6 as i64 - g.offset[1] as i64) as f64 / 64.;
            let position = [(advance + g.offset[0] as i64) as f64 / 64. - shear * y, y];
            writeln!(
                content,
                "{:.9} {:.9} Td <{:04X}> Tj",
                position[0] - previous[0],
                position[1] - previous[1],
                cid_map[&g.id]
            )
            .unwrap();
            previous = position;
            advance += g.advance[0] as i64;
        }
        content.extend_from_slice(b"ET\nQ\n");
        p.stream(Ref::new(4), &content);
        p.page(Ref::new(3))
            .parent(Ref::new(2))
            .media_box(Rect::new(0., 0., 595., 842.))
            .contents(Ref::new(4))
            .resources()
            .fonts()
            .pair(Name(b"F"), Ref::new(5));
        std::fs::write(out.join(format!("{name}.native.pdf")), p.finish()).unwrap();
    }
}
