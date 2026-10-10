// SPDX-License-Identifier: GPL-3.0-or-later
//! Classified composer view DXF, in model metres, matching formats/dxf_out.py.
//! This is deliberately separate from the 3D scene exporter.
use std::io::{self, Write};

pub struct ViewLayer<'a> {
    pub name: &'a str,
    pub segments: &'a [[f64; 4]],
    pub dashed: bool,
}

fn layer_name(name: &str) -> String {
    let name = name.trim().to_uppercase().replace(' ', "_");
    let name = if name.is_empty() { "VISTA" } else { &name };
    name.chars().map(|c| if c.is_ascii() { c } else { '?' }).collect()
}

/// Writes R12 ASCII LINE entities and declares even empty class layers.
/// Coordinates are camera-plane metres, never sheet millimetres or scaled pixels.
/// All coordinates are checked before any output is written.
pub fn write_view_dxf(mut out: impl Write, groups: &[ViewLayer<'_>]) -> io::Result<usize> {
    if groups.iter().flat_map(|g| g.segments).flatten().any(|v| !v.is_finite()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Non-finite view DXF coordinate"));
    }
    let mut names = Vec::<(String, bool)>::new();
    for group in groups {
        let name = layer_name(group.name);
        if !names.iter().any(|(n, _)| *n == name) { names.push((name, group.dashed)); }
    }
    write!(out, "0\nSECTION\n2\nTABLES\n")?;
    if names.iter().any(|(_, dashed)| *dashed) {
        write!(out, "0\nTABLE\n2\nLTYPE\n70\n1\n0\nLTYPE\n2\nDASHED\n70\n0\n3\n__ __ __ __\n72\n65\n73\n2\n40\n0.75\n49\n0.5\n49\n-0.25\n0\nENDTAB\n")?;
    }
    write!(out, "0\nTABLE\n2\nLAYER\n70\n{}\n", names.len())?;
    for (name, dashed) in &names {
        writeln!(out, "0\nLAYER\n2\n{name}\n70\n0\n62\n7\n6\n{}", if *dashed { "DASHED" } else { "CONTINUOUS" })?;
    }
    write!(out, "0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n")?;
    let mut count = 0;
    for group in groups {
        let name = layer_name(group.name);
        for [x0,y0,x1,y1] in group.segments {
            // Nine significant digits, as the original .9g writer; DXF accepts exponents.
            writeln!(out, "0\nLINE\n8\n{name}\n10\n{x0:.8e}\n20\n{y0:.8e}\n30\n0\n11\n{x1:.8e}\n21\n{y1:.8e}\n31\n0")?;
            count += 1;
        }
    }
    write!(out, "0\nENDSEC\n0\nEOF\n")?;
    Ok(count)
}
