// Source-subset intermediate experiment, not an application PDF painter.
use freetype_sys as ft;
use std::{ffi::c_void, sync::Arc};
#[derive(Clone, Debug)]
enum Op {
    Move([f64; 2]),
    Line([f64; 2]),
    Cubic([[f64; 2]; 3]),
}
#[derive(Default)]
struct Outline {
    ops: Vec<Op>,
    last: [f64; 2],
}
fn point(v: *const ft::FT_Vector) -> [f64; 2] {
    unsafe { [(*v).x as f64 / 64., (*v).y as f64 / 64.] }
}
extern "C" fn mv(v: *const ft::FT_Vector, d: *mut c_void) -> i32 {
    let o = unsafe { &mut *d.cast::<Outline>() };
    o.last = point(v);
    o.ops.push(Op::Move(o.last));
    0
}
extern "C" fn ln(v: *const ft::FT_Vector, d: *mut c_void) -> i32 {
    let o = unsafe { &mut *d.cast::<Outline>() };
    let next = point(v);
    if next != o.last {
        o.ops.push(Op::Line(next));
    }
    o.last = next;
    0
}
extern "C" fn quad(c: *const ft::FT_Vector, v: *const ft::FT_Vector, d: *mut c_void) -> i32 {
    let o = unsafe { &mut *d.cast::<Outline>() };
    let c = point(c);
    let e = point(v);
    let a = o.last;
    let first = [(a[0] + 2. * c[0]) / 3., (a[1] + 2. * c[1]) / 3.];
    let second = [(e[0] + 2. * c[0]) / 3., (e[1] + 2. * c[1]) / 3.];
    o.ops.push(Op::Cubic([first, second, e]));
    o.last = e;
    0
}
extern "C" fn cubic(
    a: *const ft::FT_Vector,
    b: *const ft::FT_Vector,
    v: *const ft::FT_Vector,
    d: *mut c_void,
) -> i32 {
    let o = unsafe { &mut *d.cast::<Outline>() };
    let e = point(v);
    o.ops.push(Op::Cubic([point(a), point(b), e]));
    o.last = e;
    0
}
#[derive(Clone, Copy)]
struct P {
    xy: [i32; 2],
    on: bool,
}
fn reduce(
    out: &mut Vec<P>,
    end: [i32; 2],
    second: [i32; 2],
    first: [i32; 2],
    start: [i32; 2],
) -> Result<(), String> {
    let mut stack = vec![[end, second, first, start]];
    let mut try_reduce = out.len() > 1 && out[out.len() - 1].on && !out[out.len() - 2].on;
    let mut split = 0;
    while let Some(v) = stack.pop() {
        let a = [
            v[1][0] + ((v[1][0] - v[0][0]) >> 1),
            v[1][1] + ((v[1][1] - v[0][1]) >> 1),
        ];
        let b = [
            v[2][0] + ((v[2][0] - v[3][0]) >> 1),
            v[2][1] + ((v[2][1] - v[3][1]) >> 1),
        ];
        if (a[0] - b[0]).abs() <= 3 && (a[1] - b[1]).abs() <= 3 {
            let xy = [(a[0] + b[0]) >> 1, (a[1] + b[1]) >> 1];
            if try_reduce {
                let prev = out[out.len() - 2].xy;
                let mid = [(prev[0] + v[2][0]) >> 1, (prev[1] + v[2][1]) >> 1];
                if (mid[0] - v[3][0]).abs() <= 3 && (mid[1] - v[3][1]).abs() <= 3 {
                    out.pop();
                }
                try_reduce = false;
            }
            out.push(P { xy, on: false });
        } else {
            split += 1;
            if split > 16 {
                return Err("source cubic subdivision bound".into());
            }
            let mid = |a: [i32; 2], b: [i32; 2]| [(a[0] + b[0]) >> 1, (a[1] + b[1]) >> 1];
            let a = mid(v[0], v[1]);
            let b = mid(v[3], v[2]);
            let c = mid(v[1], v[2]);
            let aa = mid(a, c);
            let bb = mid(b, c);
            let middle = mid(aa, bb);
            stack.push([v[0], a, aa, middle]);
            stack.push([middle, bb, b, v[3]]);
        }
    }
    Ok(())
}
fn convert(ops: &[Op], em: f64) -> Result<(Vec<[i32; 3]>, Vec<usize>), String> {
    let mut out: Vec<P> = Vec::new();
    let mut ends: Vec<usize> = Vec::new();
    let scaled = |v: [f64; 2]| {
        [
            (v[0] * 2048. / em).round() as i32,
            (v[1] * 2048. / em).round() as i32,
        ]
    };
    for (i, op) in ops.iter().enumerate() {
        match op {
            Op::Move(v) => {
                if i + 1 == ops.len() {
                    continue;
                }
                if i != 0 {
                    let start = if let Some(v) = ends.last() {
                        v.saturating_sub(1)
                    } else {
                        0
                    };
                    if out.last().unwrap().xy == out[start].xy {
                        out.pop();
                    }
                    ends.push(out.len() - 1);
                }
                out.push(P {
                    xy: scaled(*v),
                    on: true,
                });
            }
            Op::Line(v) => {
                if i + 1 < ops.len() {
                    out.push(P {
                        xy: scaled(*v),
                        on: true,
                    });
                }
            }
            Op::Cubic(v) => {
                let end = scaled(v[2]);
                let start = out.last().ok_or("missingcurveorigin")?.xy;
                reduce(&mut out, end, scaled(v[1]), scaled(v[0]), start)?;
                out.push(P { xy: end, on: true });
            }
        }
    }
    if !out.is_empty() {
        let start = ends.last().map_or(0, |v| v + 1);
        if out.last().unwrap().xy == out[start].xy {
            out.pop();
        }
        ends.push(out.len() - 1);
    }
    Ok((
        out.into_iter()
            .map(|p| [p.xy[0], p.xy[1], i32::from(p.on)])
            .collect(),
        ends,
    ))
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut cases = Vec::new();
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if ![
            "accents",
            "bold",
            "combining",
            "italic",
            "kerns",
            "latin",
            "mono",
            "serif",
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
        let shaping = ingetrazo_formats::paper_text::PaperFont::new_with_outline_policy(
            bytes.clone(),
            0,
            Some(ingetrazo_formats::paper_text::FtPolicy {
                hinting: ingetrazo_formats::paper_text::Hinting::Vertical,
                force_auto_hint: false,
            }),
        )
        .unwrap();
        let run = shaping
            .shape_run(call["text"].as_str().unwrap(), None)
            .unwrap();
        let em = shaping.units_per_em;
        unsafe {
            let mut library = std::ptr::null_mut();
            assert_eq!(ft::FT_Init_FreeType(&mut library), 0);
            let mut face = std::ptr::null_mut();
            assert_eq!(
                ft::FT_New_Memory_Face(library, bytes.as_ptr(), bytes.len() as _, 0, &mut face),
                0
            );
            assert_eq!(
                ft::FT_Set_Char_Size(face, (em as i64 * 64) as _, (em as i64 * 64) as _, 0, 0),
                0
            );
            let mut glyphs = Vec::new();
            for glyph in run {
                assert_eq!(ft::FT_Load_Glyph(face, glyph.id, ft::FT_LOAD_NO_BITMAP), 0);
                let slot = (*face).glyph;
                let mut outline = Outline::default();
                if (*slot).outline.n_points > 0 {
                    let funcs = ft::FT_Outline_Funcs {
                        move_to: mv,
                        line_to: ln,
                        conic_to: quad,
                        cubic_to: cubic,
                        shift: 0,
                        delta: 0,
                    };
                    assert_eq!(
                        ft::FT_Outline_Decompose(
                            &mut (*slot).outline,
                            &funcs,
                            (&mut outline as *mut Outline).cast()
                        ),
                        0
                    );
                }
                let (points, ends) = convert(&outline.ops, em as f64).unwrap();
                glyphs.push(serde_json::json!({"gid":glyph.id,"points":points,"ends":ends}));
            }
            assert_eq!(ft::FT_Done_Face(face), 0);
            assert_eq!(ft::FT_Done_FreeType(library), 0);
            cases.push(serde_json::json!({"case":name,"glyphs_in_draw_order":glyphs}));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"cases":cases})).unwrap()
    );
}

// Intermediate engineering encoder; production cache/budget integration pending.
pub fn subset_program(
    bytes: &[u8],
    ids: &[u32],
) -> (Vec<u8>, std::collections::BTreeMap<u32, u16>) {
    let face_info = ttf_parser::Face::parse(bytes, 0).unwrap();
    let em = face_info.units_per_em();
    let mut order = vec![0u32];
    let mut map = std::collections::BTreeMap::from([(0u32, 0u16)]);
    for id in ids {
        if !map.contains_key(id) {
            map.insert(*id, order.len() as u16);
            order.push(*id);
        }
    }
    let mut glyf = Vec::new();
    let mut loca = Vec::new();
    let mut hmtx = Vec::new();
    let mut max_points = 0;
    let mut max_contours = 0;
    let mut bounds = [i16::MAX, i16::MAX, i16::MIN, i16::MIN];
    fn short(out: &mut Vec<u8>, v: i16) {
        out.extend_from_slice(&v.to_be_bytes());
    }
    unsafe {
        let mut library = std::ptr::null_mut();
        assert_eq!(ft::FT_Init_FreeType(&mut library), 0);
        let mut face = std::ptr::null_mut();
        assert_eq!(
            ft::FT_New_Memory_Face(library, bytes.as_ptr(), bytes.len() as _, 0, &mut face),
            0
        );
        assert_eq!(
            ft::FT_Set_Char_Size(face, (em as i64 * 64) as _, (em as i64 * 64) as _, 0, 0),
            0
        );
        for id in &order {
            loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
            assert_eq!(ft::FT_Load_Glyph(face, *id, ft::FT_LOAD_NO_BITMAP), 0);
            let slot = (*face).glyph;
            let advance = ((*slot).advance.x as f64 / 64. * 2048. / em as f64).round() as u16;
            let lsb =
                ((*slot).metrics.horiBearingX as f64 / 64. * 2048. / em as f64).round() as i16;
            hmtx.extend_from_slice(&advance.to_be_bytes());
            short(&mut hmtx, lsb);
            let mut outline = Outline::default();
            if (*slot).outline.n_points > 0 {
                let funcs = ft::FT_Outline_Funcs {
                    move_to: mv,
                    line_to: ln,
                    conic_to: quad,
                    cubic_to: cubic,
                    shift: 0,
                    delta: 0,
                };
                assert_eq!(
                    ft::FT_Outline_Decompose(
                        &mut (*slot).outline,
                        &funcs,
                        (&mut outline as *mut Outline).cast()
                    ),
                    0
                );
            }
            let (points, ends) = convert(&outline.ops, em as f64).unwrap();
            max_points = max_points.max(points.len());
            max_contours = max_contours.max(ends.len());
            if points.is_empty() {
                continue;
            }
            let bbox = [
                points.iter().map(|p| p[0]).min().unwrap(),
                points.iter().map(|p| p[1]).min().unwrap(),
                points.iter().map(|p| p[0]).max().unwrap(),
                points.iter().map(|p| p[1]).max().unwrap(),
            ];
            short(&mut glyf, ends.len() as i16);
            for (i, v) in bbox.iter().enumerate() {
                let v = i16::try_from(*v).unwrap();
                short(&mut glyf, v);
                bounds[i] = if i < 2 {
                    bounds[i].min(v)
                } else {
                    bounds[i].max(v)
                };
            }
            for end in ends {
                glyf.extend_from_slice(&(end as u16).to_be_bytes());
            }
            short(&mut glyf, 0);
            for p in &points {
                glyf.push(p[2] as u8);
            }
            for axis in 0..2 {
                let mut previous = 0;
                for p in &points {
                    short(&mut glyf, i16::try_from(p[axis] - previous).unwrap());
                    previous = p[axis];
                }
            }
            while glyf.len() % 4 != 0 {
                glyf.push(0);
            }
        }
        assert_eq!(ft::FT_Done_Face(face), 0);
        assert_eq!(ft::FT_Done_FreeType(library), 0);
    }
    loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
    let raw = face_info.raw_face();
    let copy = |tag| {
        raw.table(ttf_parser::Tag::from_bytes(tag))
            .unwrap()
            .to_vec()
    };
    let mut head = copy(b"head");
    head[8..12].fill(0);
    head[16..18].fill(0);
    head[18..20].copy_from_slice(&2048u16.to_be_bytes());
    for (i, v) in bounds.iter().enumerate() {
        head[36 + 2 * i..38 + 2 * i].copy_from_slice(&v.to_be_bytes());
    }
    head[50..52].copy_from_slice(&1i16.to_be_bytes());
    let mut hhea = copy(b"hhea");
    hhea[34..36].copy_from_slice(&(order.len() as u16).to_be_bytes());
    let mut maxp = vec![0; 32];
    maxp[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    maxp[4..6].copy_from_slice(&(order.len() as u16).to_be_bytes());
    maxp[6..8].copy_from_slice(&(max_points as u16).to_be_bytes());
    maxp[8..10].copy_from_slice(&(max_contours as u16).to_be_bytes());
    let mut tables = std::collections::BTreeMap::from([
        (*b"head", head),
        (*b"hhea", hhea),
        (*b"maxp", maxp),
        (*b"glyf", glyf),
        (*b"loca", loca),
        (*b"hmtx", hmtx),
        (*b"name", copy(b"name")),
        (*b"OS/2", copy(b"OS/2")),
    ]);
    fn sum(b: &[u8]) -> u32 {
        b.chunks(4).fold(0u32, |s, v| {
            let mut a = [0; 4];
            a[..v.len()].copy_from_slice(v);
            s.wrapping_add(u32::from_be_bytes(a))
        })
    }
    let count = tables.len() as u16;
    let power = 1u16 << (15 - count.leading_zeros() as u16);
    let mut out = Vec::new();
    out.extend_from_slice(&0x10000u32.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&(power * 16).to_be_bytes());
    out.extend_from_slice(&(power.trailing_zeros() as u16).to_be_bytes());
    out.extend_from_slice(&((count - power) * 16).to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    let mut head_offset = 0;
    for (tag, bytes) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&sum(bytes).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        if tag == b"head" {
            head_offset = offset;
        }
        offset += (bytes.len() + 3) & !3;
    }
    for bytes in tables.values_mut() {
        out.append(bytes);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    let adjustment = 0xB1B0AFBAu32.wrapping_sub(sum(&out));
    out[head_offset + 8..head_offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    (out, map)
}
