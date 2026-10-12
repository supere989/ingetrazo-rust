// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Geographic Reference & Topography System (Track G).
//!
//! Provides datum projection (WGS84 / UTM), survey points parsing,
//! traced paths (GeoPath) with area and chainage profiling, 
//! DTM/TIN terrain heightfields and draped 3D surfaces.

use glam::DVec3;
use serde::{Deserialize, Serialize};

// --- WGS84 Ellipsoid & UTM Constants ---
pub const WGS84_A: f64 = 6378137.0; // semi-major axis (meters)
pub const WGS84_F: f64 = 1.0 / 298.257223563; // flattening
pub const WGS84_E2: f64 = WGS84_F * (2.0 - WGS84_F); // first eccentricity squared
pub const WGS84_EP2: f64 = WGS84_E2 / (1.0 - WGS84_E2); // second eccentricity squared
pub const UTM_K0: f64 = 0.9996; // UTM scale factor
pub const FALSE_EASTING: f64 = 500000.0;
pub const FALSE_NORTHING: f64 = 10000000.0; // southern hemisphere

/// UTM zone number (1..60) containing longitude `lon` in degrees.
pub fn zone_for_lon(lon: f64) -> i32 {
    ((lon + 180.0) / 6.0).floor() as i32 + 1
}

/// Central-meridian longitude of `zone` in radians.
pub fn central_meridian_rad(zone: i32) -> f64 {
    ((zone - 1) * 6 - 180 + 3) as f64 * std::f64::consts::PI / 180.0
}

/// Project geodetic `lat`/`lon` (degrees, WGS84) onto UTM `zone`.
/// Returns `(easting, northing)` in meters.
pub fn utm_forward(lat: f64, lon: f64, zone: i32) -> (f64, f64) {
    let latr = lat.to_radians();
    let lonr = lon.to_radians();
    let lon0 = central_meridian_rad(zone);

    let sin_lat = latr.sin();
    let cos_lat = latr.cos();
    let tan_lat = latr.tan();

    let n = WGS84_A / (1.0 - WGS84_E2 * sin_lat * sin_lat).sqrt();
    let t = tan_lat * tan_lat;
    let c = WGS84_EP2 * cos_lat * cos_lat;
    let a = cos_lat * (lonr - lon0);

    let m = WGS84_A * (
        (1.0 - WGS84_E2 / 4.0 - 3.0 * WGS84_E2.powi(2) / 64.0 - 5.0 * WGS84_E2.powi(3) / 256.0) * latr
            - (3.0 * WGS84_E2 / 8.0 + 3.0 * WGS84_E2.powi(2) / 32.0 + 45.0 * WGS84_E2.powi(3) / 1024.0) * (2.0 * latr).sin()
            + (15.0 * WGS84_E2.powi(2) / 256.0 + 45.0 * WGS84_E2.powi(3) / 1024.0) * (4.0 * latr).sin()
            - (35.0 * WGS84_E2.powi(3) / 3072.0) * (6.0 * latr).sin()
    );

    let easting = UTM_K0 * n * (
        a + (1.0 - t + c) * a.powi(3) / 6.0
            + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * WGS84_EP2) * a.powi(5) / 120.0
    ) + FALSE_EASTING;

    let mut northing = UTM_K0 * (
        m + n * tan_lat * (
            a.powi(2) / 2.0
                + (5.0 - t + 9.0 * c + 4.0 * c * c) * a.powi(4) / 24.0
                + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * WGS84_EP2) * a.powi(6) / 720.0
        )
    );

    if lat < 0.0 {
        northing += FALSE_NORTHING;
    }

    (easting, northing)
}

/// Inverse UTM projection: `(easting, northing)` -> `(lat, lon)` in degrees.
pub fn utm_inverse(easting: f64, northing: f64, zone: i32, northern: bool) -> (f64, f64) {
    let x = easting - FALSE_EASTING;
    let y = if northern { northing } else { northing - FALSE_NORTHING };

    let m = y / UTM_K0;
    let mu = m / (WGS84_A * (1.0 - WGS84_E2 / 4.0 - 3.0 * WGS84_E2.powi(2) / 64.0 - 5.0 * WGS84_E2.powi(3) / 256.0));

    let e1 = (1.0 - (1.0 - WGS84_E2).sqrt()) / (1.0 + (1.0 - WGS84_E2).sqrt());

    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1.powi(2) / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();

    let sin_phi1 = phi1.sin();
    let cos_phi1 = phi1.cos();
    let tan_phi1 = phi1.tan();

    let c1 = WGS84_EP2 * cos_phi1 * cos_phi1;
    let t1 = tan_phi1 * tan_phi1;
    let n1 = WGS84_A / (1.0 - WGS84_E2 * sin_phi1 * sin_phi1).sqrt();
    let r1 = WGS84_A * (1.0 - WGS84_E2) / (1.0 - WGS84_E2 * sin_phi1 * sin_phi1).powf(1.5);
    let d = x / (n1 * UTM_K0);

    let lat = phi1 - (n1 * tan_phi1 / r1) * (
        d * d / 2.0
            - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * WGS84_EP2) * d.powi(4) / 24.0
            + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * WGS84_EP2 - 3.0 * c1 * c1) * d.powi(6) / 720.0
    );

    let lon = central_meridian_rad(zone) + (
        d - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * WGS84_EP2 + 24.0 * t1 * t1) * d.powi(5) / 120.0
    ) / cos_phi1;

    (lat.to_degrees(), lon.to_degrees())
}

/// Fold an angle into (-180, 180] degrees so 350° and -10° are the same north.
pub fn wrap_deg(deg: f64) -> f64 {
    let d = (deg + 180.0).rem_euclid(360.0) - 180.0;
    if d == -180.0 { 180.0 } else { d }
}

/// Scene Datum: anchors local scene meters to geographic coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneDatum {
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    pub north: f64,
    pub zone: i32,
    pub northern: bool,
    #[serde(skip)]
    pub east0: f64,
    #[serde(skip)]
    pub north0: f64,
    #[serde(skip)]
    pub cos_n: f64,
    #[serde(skip)]
    pub sin_n: f64,
}

impl SceneDatum {
    pub fn new(lat: f64, lon: f64, alt: f64, north: f64) -> Self {
        let n_wrapped = wrap_deg(north);
        let zone = zone_for_lon(lon);
        let northern = lat >= 0.0;
        let (east0, north0) = utm_forward(lat, lon, zone);
        let b = n_wrapped.to_radians();
        Self {
            lat,
            lon,
            alt,
            north: n_wrapped,
            zone,
            northern,
            east0,
            north0,
            cos_n: b.cos(),
            sin_n: b.sin(),
        }
    }

    pub fn with_forced_zone(mut self, zone: i32, northern: bool) -> Self {
        self.zone = zone;
        self.northern = northern;
        let (e0, n0) = utm_forward(self.lat, self.lon, zone);
        self.east0 = e0;
        self.north0 = n0;
        self
    }

    pub fn grid_to_local_xy(&self, east: f64, north: f64) -> (f64, f64) {
        let c = self.cos_n;
        let s = self.sin_n;
        (east * c + north * s, north * c - east * s)
    }

    pub fn local_to_grid_xy(&self, x: f64, y: f64) -> (f64, f64) {
        let c = self.cos_n;
        let s = self.sin_n;
        (x * c - y * s, x * s + y * c)
    }

    pub fn north_vector(&self) -> DVec3 {
        DVec3::new(self.sin_n, self.cos_n, 0.0)
    }

    pub fn hemisphere(&self) -> &'static str {
        if self.northern { "N" } else { "S" }
    }

    pub fn geodetic_to_local(&self, lat: f64, lon: f64, alt: Option<f64>) -> DVec3 {
        let (east, north) = utm_forward(lat, lon, self.zone);
        let z = alt.map(|a| a - self.alt).unwrap_or(0.0);
        let (x, y) = self.grid_to_local_xy(east - self.east0, north - self.north0);
        DVec3::new(x, y, z)
    }

    pub fn local_to_geodetic(&self, point: DVec3) -> (f64, f64, f64) {
        let (de, dn) = self.local_to_grid_xy(point.x, point.y);
        let (lat, lon) = utm_inverse(de + self.east0, dn + self.north0, self.zone, self.northern);
        (lat, lon, point.z + self.alt)
    }

    pub fn utm_to_local(&self, east: f64, north: f64, alt: f64) -> DVec3 {
        let (x, y) = self.grid_to_local_xy(east - self.east0, north - self.north0);
        DVec3::new(x, y, alt - self.alt)
    }

    pub fn local_to_utm(&self, point: DVec3) -> (f64, f64, f64) {
        let (de, dn) = self.local_to_grid_xy(point.x, point.y);
        (de + self.east0, dn + self.north0, point.z + self.alt)
    }

    /// Refresh cached trig and origin coordinates after deserialization.
    pub fn rebuild_cache(&mut self) {
        let (e0, n0) = utm_forward(self.lat, self.lon, self.zone);
        self.east0 = e0;
        self.north0 = n0;
        let b = self.north.to_radians();
        self.cos_n = b.cos();
        self.sin_n = b.sin();
    }
}

// --- Survey Points ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
    pub position: DVec3,
    pub name: String,
    pub desc: String,
}

impl GeoPoint {
    pub fn new(position: DVec3, name: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            position,
            name: name.into(),
            desc: desc.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SurveyPointRow {
    pub name: String,
    pub north: f64,
    pub east: f64,
    pub z: f64,
    pub desc: String,
}

fn parse_coord(cell: &str) -> Result<f64, std::num::ParseFloatError> {
    cell.trim().replace(',', ".").parse::<f64>()
}

/// Parse survey-CSV text into rows of P,N,E,Z[,D].
pub fn parse_points_csv(text: &str) -> Result<Vec<SurveyPointRow>, String> {
    let mut rows = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // The original parses each physical line with csv.reader, accepting
        // escaped quotes and delimiters inside descriptions, not a split(',').
        let cells: Vec<String> = if let Some(delimiter) = if line.contains(';') {Some(';')} else if line.contains(',') {Some(',')} else {None} {
            let mut cells=Vec::new();let mut field=String::new();let mut quoted=false;let mut begin=true;let mut chars=line.chars().peekable();
            while let Some(c)=chars.next(){
                if quoted {if c=='"' {if chars.peek()==Some(&'"'){chars.next();field.push('"');}else{quoted=false;}}else{field.push(c);}}
                else if c==delimiter {cells.push(field.trim().to_owned());field.clear();begin=true;}
                else if c=='"'&&begin {quoted=true;begin=false;}
                else {field.push(c);begin=false;}
            }
            cells.push(field.trim().to_owned());cells
        } else {line.split_whitespace().map(str::to_owned).collect()};

        if cells.len() < 4 {
            continue;
        }

        let north = match parse_coord(&cells[1]) {
            Ok(v) => v,
            Err(_) => continue, // likely header row
        };
        let east = match parse_coord(&cells[2]) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let z = match parse_coord(&cells[3]) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let desc = if cells.len() > 4 {
            cells[4..].join(",").trim().to_string()
        } else {
            String::new()
        };

        rows.push(SurveyPointRow {
            name: cells[0].to_string(),
            north,
            east,
            z,
            desc,
        });
    }

    if rows.is_empty() {
        return Err("No survey points found (expected P,N,E,Z[,desc])".to_string());
    }

    Ok(rows)
}

pub fn datum_for_rows(rows: &[SurveyPointRow], zone: i32, northern: bool) -> SceneDatum {
    let first = &rows[0];
    let (lat, lon) = utm_inverse(first.east, first.north, zone, northern);
    let mut datum = SceneDatum::new(lat, lon, first.z, 0.0);
    if datum.zone != zone {
        datum = datum.with_forced_zone(zone, northern);
    }
    datum
}

pub fn points_from_rows(rows: &[SurveyPointRow], datum: &SceneDatum) -> Vec<GeoPoint> {
    rows.iter()
        .map(|r| GeoPoint::new(datum.utm_to_local(r.east, r.north, r.z), &r.name, &r.desc))
        .collect()
}

// --- Traced Polyline (GeoPath) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceFillMode {
    Flat,
    Draped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeoPath {
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
    pub points: Vec<DVec3>,
    pub closed: bool,
    pub name: String,
    pub surface: Option<SurfaceFillMode>,
    #[serde(skip)]
    pub surface_tris: Option<Vec<(DVec3, DVec3, DVec3)>>,
}

impl GeoPath {
    pub fn new(points: Vec<DVec3>, closed: bool, name: impl Into<String>) -> Self {
        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            points,
            closed,
            name: name.into(),
            surface: None,
            surface_tris: None,
        }
    }

    pub fn segments(&self) -> Vec<(DVec3, DVec3)> {
        let mut segs = Vec::new();
        if self.points.len() < 2 {
            return segs;
        }
        for i in 0..self.points.len() - 1 {
            segs.push((self.points[i], self.points[i + 1]));
        }
        if self.closed && self.points.len() > 2 {
            segs.push((*self.points.last().unwrap(), self.points[0]));
        }
        segs
    }

    /// Horizontal (XY) length along the path.
    pub fn length(&self) -> f64 {
        self.segments()
            .iter()
            .map(|(a, b)| (b.x - a.x).hypot(b.y - a.y))
            .sum()
    }

    pub fn perimeter(&self) -> f64 {
        self.length()
    }

    /// Planimetric (XY-projected) area of a closed polygon; 0.0 if open or < 3 nodes.
    pub fn area(&self) -> f64 {
        if !self.closed || self.points.len() < 3 {
            return 0.0;
        }
        let n = self.points.len();
        let mut s = 0.0;
        for i in 0..n {
            let next = (i + 1) % n;
            s += self.points[i].x * self.points[next].y - self.points[next].x * self.points[i].y;
        }
        (s.abs()) * 0.5
    }

    /// Real 3D area of the generated terrain surface, or None if no surface triangles exist.
    pub fn surface_area(&self) -> Option<f64> {
        self.surface_tris.as_ref().map(|tris| {
            tris.iter()
                .map(|(a, b, c)| (*b - *a).cross(*c - *a).length() * 0.5)
                .sum()
        })
    }

    pub fn profile_points(&self) -> Vec<DVec3> {
        let mut pts = self.points.clone();
        if self.closed && pts.len() > 2 {
            pts.push(pts[0]);
        }
        pts
    }
}

// --- Longitudinal Profile ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSample {
    pub station: f64,
    pub x: f64,
    pub y: f64,
    pub elevation: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub samples: Vec<ProfileSample>,
    pub length: f64,
    pub complete: bool,
}

impl Profile {
    pub fn elevations(&self) -> Vec<f64> {
        self.samples.iter().filter_map(|s| s.elevation).collect()
    }

    pub fn min_elevation(&self) -> Option<f64> {
        self.elevations().into_iter().reduce(f64::min)
    }

    pub fn max_elevation(&self) -> Option<f64> {
        self.elevations().into_iter().reduce(f64::max)
    }

    pub fn total_gain(&self) -> f64 {
        let mut gain = 0.0;
        let mut prev = None;
        for s in &self.samples {
            if let Some(e) = s.elevation {
                if let Some(p) = prev {
                    if e > p {
                        gain += e - p;
                    }
                }
                prev = Some(e);
            }
        }
        gain
    }

    pub fn slopes(&self) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        for i in 0..self.samples.len().saturating_sub(1) {
            let a = &self.samples[i];
            let b = &self.samples[i + 1];
            if let (Some(ea), Some(eb)) = (a.elevation, b.elevation) {
                let ds = b.station - a.station;
                if ds > 1e-9 {
                    out.push((a.station, (eb - ea) / ds * 100.0));
                }
            }
        }
        out
    }
}

pub fn point_at_station(pts: &[DVec3], s: f64) -> (f64, f64) {
    if pts.is_empty() {
        return (0.0, 0.0);
    }
    if s <= 0.0 {
        return (pts[0].x, pts[0].y);
    }
    let mut acc = 0.0;
    for i in 0..pts.len() - 1 {
        let a = pts[i];
        let b = pts[i + 1];
        let seg = (b.x - a.x).hypot(b.y - a.y);
        if acc + seg >= s {
            let t = if seg > 1e-12 { (s - acc) / seg } else { 0.0 };
            return (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        }
        acc += seg;
    }
    let last = pts.last().unwrap();
    (last.x, last.y)
}

pub fn sample_profile_with_elevation<F: Fn(f64, f64) -> Option<f64>>(
    polyline: &[DVec3],
    elevation_fn: F,
    spacing: Option<f64>,
) -> Profile {
    if polyline.len() < 2 {
        return Profile {
            samples: Vec::new(),
            length: 0.0,
            complete: true,
        };
    }
    let mut length = 0.0;
    for i in 0..polyline.len() - 1 {
        length += (polyline[i + 1].x - polyline[i].x).hypot(polyline[i + 1].y - polyline[i].y);
    }
    if length <= 1e-9 {
        return Profile {
            samples: Vec::new(),
            length: 0.0,
            complete: true,
        };
    }

    let step = spacing.unwrap_or_else(|| (length / 256.0).max(1.0));
    let n = ((length / step).ceil() as usize).max(1);

    let mut samples = Vec::with_capacity(n + 1);
    let mut complete = true;

    for i in 0..=n {
        let s = (i as f64 * step).min(length);
        let (x, y) = point_at_station(polyline, s);
        let elevation = elevation_fn(x, y);
        if elevation.is_none() {
            complete = false;
        }
        samples.push(ProfileSample {
            station: s,
            x,
            y,
            elevation,
        });
    }

    Profile {
        samples,
        length,
        complete,
    }
}

// --- Slippy Map / Web Mercator Math ---

pub const MAX_WEB_MERCATOR_LAT: f64 = 85.05112878;

pub fn deg2num(lat_deg: f64, lon_deg: f64, zoom: u8) -> (f64, f64) {
    let lat_clamped = lat_deg.clamp(-MAX_WEB_MERCATOR_LAT, MAX_WEB_MERCATOR_LAT);
    let lat_rad = lat_clamped.to_radians();
    let n = 2.0_f64.powi(zoom as i32);
    let xtile = (lon_deg + 180.0) / 360.0 * n;
    let ytile = (1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n;
    (xtile, ytile)
}

pub fn num2deg(xtile: f64, ytile: f64, zoom: u8) -> (f64, f64) {
    let n = 2.0_f64.powi(zoom as i32);
    let lon_deg = xtile / n * 360.0 - 180.0;
    let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * ytile / n)).sinh().atan();
    (lat_rad.to_degrees(), lon_deg)
}

pub fn tile_bbox(x: u32, y: u32, zoom: u8) -> (f64, f64, f64, f64) {
    let (lat_n, lon_w) = num2deg(x as f64, y as f64, zoom);
    let (lat_s, lon_e) = num2deg((x + 1) as f64, (y + 1) as f64, zoom);
    (lat_s, lon_w, lat_n, lon_e)
}

pub fn tiles_covering(lat_s: f64, lon_w: f64, lat_n: f64, lon_e: f64, zoom: u8) -> Vec<(u32, u32)> {
    let (x0_f, y0_f) = deg2num(lat_n, lon_w, zoom);
    let (x1_f, y1_f) = deg2num(lat_s, lon_e, zoom);

    let n = 2_u32.pow(zoom as u32);
    let x_min = (x0_f.floor() as i64).clamp(0, (n - 1) as i64) as u32;
    let x_max = (x1_f.floor() as i64).clamp(0, (n - 1) as i64) as u32;
    let y_min = (y0_f.floor() as i64).clamp(0, (n - 1) as i64) as u32;
    let y_max = (y1_f.floor() as i64).clamp(0, (n - 1) as i64) as u32;

    let mut tiles = Vec::new();
    for y in y_min..=y_max {
        for x in x_min..=x_max {
            tiles.push((x, y));
        }
    }
    tiles
}

// --- Terrain Object & Surface Mesh Generation ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainObject {
    #[serde(skip,default="uuid::Uuid::new_v4")]
    pub history_id:uuid::Uuid,
    pub vertices: Vec<DVec3>,
    pub uvs: Vec<(f64, f64)>,
    pub triangles: Vec<(usize, usize, usize)>,
    pub tile_range: (u32, u32, u32, u32, u8), // tx0, ty0, ntx, nty, zoom
    pub nx: usize,
    pub ny: usize,
    pub bbox: (f64, f64, f64, f64), // minx, miny, maxx, maxy
    pub visible: bool,
}

impl TerrainObject {
    pub fn height_at(&self, x: f64, y: f64) -> Option<f64> {
        let (minx, miny, maxx, maxy) = self.bbox;
        if self.nx < 2 || self.ny < 2 || maxx <= minx || maxy <= miny {
            return None;
        }
        if x < minx || x > maxx || y < miny || y > maxy {
            return None;
        }

        let fi = (x - minx) / (maxx - minx) * (self.nx - 1) as f64;
        let fj = (maxy - y) / (maxy - miny) * (self.ny - 1) as f64; // row 0 is north (+Y)

        let i0 = (fi.floor() as usize).min(self.nx - 2);
        let j0 = (fj.floor() as usize).min(self.ny - 2);

        let tx = fi - i0 as f64;
        let ty = fj - j0 as f64;

        let z00 = self.vertices[j0 * self.nx + i0].z;
        let z10 = self.vertices[j0 * self.nx + i0 + 1].z;
        let z01 = self.vertices[(j0 + 1) * self.nx + i0].z;
        let z11 = self.vertices[(j0 + 1) * self.nx + i0 + 1].z;

        let top = z00 * (1.0 - tx) + z10 * tx;
        let bot = z01 * (1.0 - tx) + z11 * tx;
        Some(top * (1.0 - ty) + bot * ty)
    }

    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        if self.vertices.is_empty() {
            return None;
        }
        let mut min = self.vertices[0];
        let mut max = self.vertices[0];
        for v in &self.vertices[1..] {
            min = min.min(*v);
            max = max.max(*v);
        }
        Some((min, max))
    }
}

/// Least-squares plane fitting: `z = a*x + b*y + c` through `(x, y, z)` samples.
pub fn fit_plane(pts: &[DVec3], zs: &[f64]) -> (f64, f64, f64) {
    let n = pts.len() as f64;
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    let mut sxy = 0.0;
    let mut sz = 0.0;
    let mut sxz = 0.0;
    let mut syz = 0.0;

    for (p, &z) in pts.iter().zip(zs) {
        let x = p.x;
        let y = p.y;
        sx += x;
        sy += y;
        sxx += x * x;
        syy += y * y;
        sxy += x * y;
        sz += z;
        sxz += x * z;
        syz += y * z;
    }

    let a_mat = [
        [sxx, sxy, sx],
        [sxy, syy, sy],
        [sx, sy, n],
    ];
    let b_vec = [sxz, syz, sz];

    let det3 = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };

    let d = det3(a_mat);
    if d.abs() < 1e-12 {
        let avg = if n > 0.0 { sz / n } else { 0.0 };
        return (0.0, 0.0, avg);
    }

    let col = |i: usize| {
        let mut m = a_mat;
        for r in 0..3 {
            m[r][i] = b_vec[r];
        }
        m
    };

    (det3(col(0)) / d, det3(col(1)) / d, det3(col(2)) / d)
}

/// Build flat single-slope plane surface over polygon.
pub fn build_flat_surface(
    pts: &[DVec3],
    zs: &[f64],
    ground_ref: f64,
) -> Option<Vec<(DVec3, DVec3, DVec3)>> {
    if pts.len() < 3 || pts.len() != zs.len() {
        return None;
    }

    let (a, b, c) = fit_plane(pts, zs);
    let mut flat_coords = Vec::with_capacity(pts.len() * 2);
    for p in pts {
        flat_coords.push(p.x);
        flat_coords.push(p.y);
    }

    let indices = earcutr::earcut(&flat_coords, &[], 2).ok()?;
    let verts: Vec<DVec3> = pts
        .iter()
        .map(|p| DVec3::new(p.x, p.y, a * p.x + b * p.y + c - ground_ref))
        .collect();

    let mut tris = Vec::with_capacity(indices.len() / 3);
    for chunk in indices.chunks_exact(3) {
        tris.push((verts[chunk[0]], verts[chunk[1]], verts[chunk[2]]));
    }
    Some(tris)
}

fn subdivide_tri(
    tri: ((f64, f64), (f64, f64), (f64, f64)),
    depth: usize,
    out: &mut Vec<((f64, f64), (f64, f64), (f64, f64))>,
) {
    if depth == 0 {
        out.push(tri);
        return;
    }
    let (a, b, c) = tri;
    let ab = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let bc = ((b.0 + c.0) * 0.5, (b.1 + c.1) * 0.5);
    let ca = ((c.0 + a.0) * 0.5, (c.1 + a.1) * 0.5);

    subdivide_tri((a, ab, ca), depth - 1, out);
    subdivide_tri((ab, b, bc), depth - 1, out);
    subdivide_tri((ca, bc, c), depth - 1, out);
    subdivide_tri((ab, bc, ca), depth - 1, out);
}

/// Polygon interior subdivided and draped onto DEM relief.
pub fn build_draped_surface<F: Fn(f64, f64) -> Option<f64>>(
    pts: &[DVec3],
    elevation_fn: F,
    ground_ref: f64,
    target_edge_m: f64,
) -> Option<Vec<(DVec3, DVec3, DVec3)>> {
    if pts.len() < 3 {
        return None;
    }

    let mut flat_coords = Vec::with_capacity(pts.len() * 2);
    for p in pts {
        flat_coords.push(p.x);
        flat_coords.push(p.y);
    }

    let indices = earcutr::earcut(&flat_coords, &[], 2).ok()?;
    if indices.is_empty() {
        return None;
    }

    let mut base_tris = Vec::with_capacity(indices.len() / 3);
    let mut longest: f64 = 0.0;
    for chunk in indices.chunks_exact(3) {
        let a = (pts[chunk[0]].x, pts[chunk[0]].y);
        let b = (pts[chunk[1]].x, pts[chunk[1]].y);
        let c = (pts[chunk[2]].x, pts[chunk[2]].y);
        longest = longest
            .max((b.0 - a.0).hypot(b.1 - a.1))
            .max((c.0 - b.0).hypot(c.1 - b.1))
            .max((a.0 - c.0).hypot(a.1 - c.1));
        base_tris.push((a, b, c));
    }

    let depth = if longest > target_edge_m {
        ((longest / target_edge_m).log2().ceil() as usize).min(5)
    } else {
        0
    };

    let mut sub_tris = Vec::new();
    for t in base_tris {
        subdivide_tri(t, depth, &mut sub_tris);
    }

    let mut out = Vec::with_capacity(sub_tris.len());
    for (a, b, c) in sub_tris {
        let ea = elevation_fn(a.0, a.1)?;
        let eb = elevation_fn(b.0, b.1)?;
        let ec = elevation_fn(c.0, c.1)?;
        out.push((
            DVec3::new(a.0, a.1, ea - ground_ref),
            DVec3::new(b.0, b.1, eb - ground_ref),
            DVec3::new(c.0, c.1, ec - ground_ref),
        ));
    }
    Some(out)
}

/// DEM-lifted terrain grid over `bbox` (minx, miny, maxx, maxy).
pub fn build_terrain_mesh<F: Fn(f64, f64) -> Option<f64>>(
    datum: &SceneDatum,
    bbox: (f64, f64, f64, f64),
    elevation_fn: F,
    ground_ref: f64,
    target_cell_m: f64,
    max_grid: usize,
    zoom: u8,
) -> Option<TerrainObject> {
    let (minx, miny, maxx, maxy) = bbox;
    let w = maxx - minx;
    let h = maxy - miny;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }

    let nx = ((w / target_cell_m).round() as usize + 1).clamp(2, max_grid);
    let ny = ((h / target_cell_m).round() as usize + 1).clamp(2, max_grid);

    // Geodetic corners
    let corners = [
        datum.local_to_geodetic(DVec3::new(minx, miny, 0.0)),
        datum.local_to_geodetic(DVec3::new(maxx, miny, 0.0)),
        datum.local_to_geodetic(DVec3::new(maxx, maxy, 0.0)),
        datum.local_to_geodetic(DVec3::new(minx, maxy, 0.0)),
    ];
    let lat_s = corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
    let lat_n = corners.iter().map(|c| c.0).fold(f64::NEG_INFINITY, f64::max);
    let lon_w = corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    let lon_e = corners.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);

    let tiles = tiles_covering(lat_s, lon_w, lat_n, lon_e, zoom);
    if tiles.is_empty() {
        return None;
    }

    let tx0 = tiles.iter().map(|t| t.0).min().unwrap();
    let ty0 = tiles.iter().map(|t| t.1).min().unwrap();
    let ntx = tiles.iter().map(|t| t.0).max().unwrap() - tx0 + 1;
    let nty = tiles.iter().map(|t| t.1).max().unwrap() - ty0 + 1;

    let mut verts = Vec::with_capacity(nx * ny);
    let mut uvs = Vec::with_capacity(nx * ny);

    for j in 0..ny {
        let y = maxy - h * (j as f64) / ((ny - 1) as f64);
        for i in 0..nx {
            let x = minx + w * (i as f64) / ((nx - 1) as f64);
            let e = elevation_fn(x, y)?;
            verts.push(DVec3::new(x, y, e - ground_ref));
            let (lat, lon, _) = datum.local_to_geodetic(DVec3::new(x, y, 0.0));
            let (xf, yf) = deg2num(lat, lon, zoom);
            uvs.push(((xf - tx0 as f64) / ntx as f64, (yf - ty0 as f64) / nty as f64));
        }
    }

    let mut triangles = Vec::with_capacity((nx - 1) * (ny - 1) * 2);
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let a = j * nx + i;
            let b = a + 1;
            let c = a + nx;
            let d = c + 1;
            triangles.push((a, c, b));
            triangles.push((b, c, d));
        }
    }

    Some(TerrainObject {
        history_id:uuid::Uuid::new_v4(),
        vertices: verts,
        uvs,
        triangles,
        tile_range: (tx0, ty0, ntx, nty, zoom),
        nx,
        ny,
        bbox,
        visible: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMA: (f64, f64) = (-12.0464, -77.0428);
    const QUITO: (f64, f64) = (-0.1807, -78.4678);
    const OSLO: (f64, f64) = (59.9139, 10.7522);

    #[test]
    fn test_zone_for_lon() {
        assert_eq!(zone_for_lon(-77.0428), 18);
        assert_eq!(zone_for_lon(-78.4678), 17);
        assert_eq!(zone_for_lon(10.7522), 32);
        assert_eq!(zone_for_lon(-180.0), 1);
        assert_eq!(zone_for_lon(179.9), 60);
    }

    #[test]
    fn test_datum_derives_zone_and_hemisphere() {
        let d = SceneDatum::new(LIMA.0, LIMA.1, 0.0, 0.0);
        assert_eq!(d.zone, 18);
        assert_eq!(d.hemisphere(), "S");
        assert!(!d.northern);

        let n = SceneDatum::new(OSLO.0, OSLO.1, 0.0, 0.0);
        assert_eq!(n.zone, 32);
        assert_eq!(n.hemisphere(), "N");
        assert!(n.northern);
    }

    #[test]
    fn test_utm_forward_inverse_round_trip() {
        for &(lat, lon) in &[LIMA, QUITO, OSLO] {
            let zone = zone_for_lon(lon);
            let (east, north) = utm_forward(lat, lon, zone);
            let (rlat, rlon) = utm_inverse(east, north, zone, lat >= 0.0);
            assert!((rlat - lat).abs() < 1e-8, "lat mismatch: {} vs {}", rlat, lat);
            assert!((rlon - lon).abs() < 1e-8, "lon mismatch: {} vs {}", rlon, lon);
        }
    }

    #[test]
    fn test_round_trip_under_1mm_city_scale() {
        for &anchor in &[LIMA, QUITO, OSLO] {
            let d = SceneDatum::new(anchor.0, anchor.1, 37.5, 0.0);
            for dn in &[-10_000.0, -100.0, 0.0, 250.0, 10_000.0] {
                for de in &[-10_000.0, -50.0, 0.0, 500.0, 10_000.0] {
                    let dlat = dn / 111_320.0;
                    let dlon = de / (111_320.0 * anchor.0.to_radians().cos());
                    let lat = anchor.0 + dlat;
                    let lon = anchor.1 + dlon;

                    let local = d.geodetic_to_local(lat, lon, Some(37.5));
                    let (rlat, rlon, ralt) = d.local_to_geodetic(local);
                    let back = d.geodetic_to_local(rlat, rlon, Some(ralt));

                    assert!((local - back).length() < 1e-3);
                    assert!((ralt - 37.5).abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn test_north_angle_rotation() {
        let plain = SceneDatum::new(-15.459809, -72.947774, 0.0, 0.0);
        let (lat, lon, _) = plain.local_to_geodetic(DVec3::new(0.0, 10.0, 0.0));

        let east = SceneDatum::new(-15.459809, -72.947774, 0.0, 90.0);
        let p = east.geodetic_to_local(lat, lon, None);
        assert!((p.x - 10.0).abs() < 1e-3);
        assert!(p.y.abs() < 1e-3);

        let nvec = east.north_vector();
        assert!((nvec.x - 1.0).abs() < 1e-3);
        assert!(nvec.y.abs() < 1e-3);
    }

    #[test]
    fn test_survey_point_csv_parsing() {
        let csv = "P,N,E,Z,DESC\nE1,8503000.000,177000.000,3335.20,ESTACION\nV1,8503012.500,177008.250,3336.10,VERTICE LOTE\nV2,8503020.000,176995.000,3334.85,VERTICE LOTE\n";
        let rows = parse_points_csv(csv).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].name, "E1");
        assert_eq!(rows[0].desc, "ESTACION");

        let datum = datum_for_rows(&rows, 19, false);
        let pts = points_from_rows(&rows, &datum);
        assert!(pts[0].position.length() < 1e-3);

        let delta = pts[1].position - pts[0].position;
        assert!((delta.x - 8.25).abs() < 1e-6);
        assert!((delta.y - 12.5).abs() < 1e-6);
        assert!((delta.z - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_geopath_area_and_length() {
        let sq = GeoPath::new(
            vec![
                DVec3::new(0.0, 0.0, 0.0),
                DVec3::new(10.0, 0.0, 0.0),
                DVec3::new(10.0, 10.0, 0.0),
                DVec3::new(0.0, 10.0, 0.0),
            ],
            true,
            "Square",
        );
        assert!((sq.length() - 40.0).abs() < 1e-9);
        assert!((sq.area() - 100.0).abs() < 1e-9);

        let open = GeoPath::new(
            vec![
                DVec3::new(0.0, 0.0, 0.0),
                DVec3::new(3.0, 0.0, 0.0),
                DVec3::new(3.0, 4.0, 0.0),
            ],
            false,
            "Line",
        );
        assert!((open.length() - 7.0).abs() < 1e-9);
        assert_eq!(open.area(), 0.0);
    }

    #[test]
    fn test_profile_sampling() {
        let polyline = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(100.0, 0.0, 0.0),
        ];
        let prof = sample_profile_with_elevation(&polyline, |x, _| Some(10.0 + x * 0.1), Some(10.0));
        assert!(prof.complete);
        assert_eq!(prof.samples.len(), 11);
        assert!((prof.total_gain() - 10.0).abs() < 1e-6);
        let slopes = prof.slopes();
        assert_eq!(slopes.len(), 10);
        for (_, slope) in slopes {
            assert!((slope - 10.0).abs() < 1e-6); // 10% slope
        }
    }
}

/// Bake selected georeferenced paths into welded drawing geometry atomically.
/// Closed planar footprints detect faces; paths are consumed only after all
/// coordinates and resulting geometry have passed validation.
pub fn geopath_to_geometry(scene:&mut crate::scene::Scene,paths:&[usize])->Result<usize,String>{
    let mut ids=paths.to_vec();ids.sort_unstable();ids.dedup();
    if ids.is_empty(){return Err("Select a path to convert to geometry".into());}
    if ids.iter().any(|&i|i>=scene.geo_paths.len()){return Err("Invalid selected path".into());}
    if ids.iter().any(|&i|scene.geo_paths[i].points.iter().any(|p|!p.is_finite())){return Err("Path coordinates must be finite".into());}
    let mut candidate=scene.clone();let mut added=0;
    for &id in &ids {
        let path=&scene.geo_paths[id];
        let mut new_edges=vec![];
        for (a,b) in path.segments(){if a.distance(b)<1e-9{continue;}let a=candidate.mesh.add_vertex(a);let b=candidate.mesh.add_vertex(b);if candidate.mesh.find_edge(a,b).is_none(){let edge=candidate.mesh.add_edge(a,b).ok_or("Path edge collapsed at document precision")?;new_edges.push(edge);added+=1;}}
        candidate.mesh=crate::planar::split_edge_intersections(&candidate.mesh,&new_edges)?;
        let mut normal=crate::math::newell_vector(&path.points).normalize_or_zero();
        // Self-crossing closed traces can have zero signed Newell area while
        // enclosing real arrangement cells (the two lobes of a bow-tie).
        if normal.length_squared()<0.5&&path.points.len()>=3 {for i in 1..path.points.len()-1 {let n=(path.points[i]-path.points[0]).cross(path.points[i+1]-path.points[0]).normalize_or_zero();if n.length_squared()>0.5{normal=n;break;}}}
        let plane=if normal.length_squared()>0.5&&path.points.iter().all(|&p|(p-path.points[0]).dot(normal).abs()<1e-6){Some((path.points[0],normal))}else{
            candidate.mesh.faces.iter().enumerate().find_map(|(i,f)|{let p=candidate.mesh.vertices[f.vertices[0]].position;let n=candidate.mesh.face_normal(i);if path.points.iter().all(|&x|(x-p).dot(n).abs()<1e-6){Some((p,n))}else{None}})
        };
        if let Some((point,normal))=plane {
            let selected:Vec<_>=candidate.mesh.faces.iter().enumerate().filter(|(_,f)|f.vertices.iter().all(|&v|(candidate.mesh.vertices[v].position-point).dot(normal).abs()<1e-6)).map(|(i,_)|i).collect();
            if !selected.is_empty()||candidate.mesh.faces.is_empty(){candidate.mesh=crate::planar::rebuild_faces(&candidate.mesh,&selected)?;} else {
                let mut plane_mesh=crate::mesh::Mesh::new();
                for e in &candidate.mesh.edges {let a=candidate.mesh.vertices[e.v0].position;let b=candidate.mesh.vertices[e.v1].position;if (a-point).dot(normal).abs()<1e-6&&(b-point).dot(normal).abs()<1e-6 {let a=plane_mesh.add_vertex(a);let b=plane_mesh.add_vertex(b);plane_mesh.add_edge(a,b);}}
                let rebuilt=crate::planar::rebuild_faces(&plane_mesh,&[])?;
                for face in &rebuilt.faces {crate::planar::add_face(&mut candidate.mesh,&crate::planar::rings(&rebuilt,face),face)?;}
            }
        }
    }
    crate::heal::refresh_interior(&mut candidate.mesh)?;
    for &i in ids.iter().rev(){candidate.geo_paths.remove(i);}
    candidate.clear_selection();candidate.bump_version();*scene=candidate;Ok(added)
}

#[cfg(test)]
mod conversion_tests {
    use super::*;use crate::scene::{Scene,SelectionItem};
    #[test]fn selected_footprint_bakes_welded_face_and_open_trace_keeps_existing_geometry(){
        let mut scene=Scene::new();scene.geo_paths.push(GeoPath::new(vec![DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y],true,"footprint"));scene.geo_paths.push(GeoPath::new(vec![DVec3::Z,DVec3::new(2.,0.,1.)],false,"line"));
        scene.select(SelectionItem::GeoPath(0));assert_eq!(geopath_to_geometry(&mut scene,&[0]).unwrap(),4);assert_eq!(scene.mesh.faces.len(),1);assert_eq!(scene.geo_paths.len(),1);assert!((scene.mesh.face_area(0)-1.).abs()<1e-9);
        let ids:Vec<_>=scene.mesh.vertices.iter().map(|v|v.uid).collect();geopath_to_geometry(&mut scene,&[0]).unwrap();assert_eq!(scene.mesh.faces.len(),1);assert_eq!(scene.mesh.edges.len(),5);assert!(ids.iter().all(|id|scene.mesh.vertices.iter().any(|v|v.uid==*id)));assert!(scene.geo_paths.is_empty());
    }
    #[test]fn crossing_trace_splits_both_wires_and_preserves_source_identity(){
        let mut scene=Scene::new();let a=scene.mesh.add_vertex(-DVec3::X);let b=scene.mesh.add_vertex(DVec3::X);let edge=scene.mesh.add_edge(a,b).unwrap();scene.mesh.edges[edge].attrs.insert("source".into(),serde_json::json!("survey"));let uid=scene.mesh.vertices[a].uid;
        scene.geo_paths.push(GeoPath::new(vec![-DVec3::Y,DVec3::Y],false,"cross"));geopath_to_geometry(&mut scene,&[0]).unwrap();
        assert_eq!(scene.mesh.edges.len(),4);assert!(scene.mesh.faces.is_empty());assert_eq!(scene.mesh.edges.iter().filter(|e|e.attrs.get("source")==Some(&serde_json::json!("survey"))).count(),2);assert!(scene.mesh.vertices.iter().any(|v|v.uid==uid&&v.position==-DVec3::X));let center=scene.mesh.vertices.iter().position(|v|v.position.length()<1e-9).unwrap();assert_eq!(scene.mesh.edges.iter().filter(|e|e.v0==center||e.v1==center).count(),4);
    }
    #[test]fn projected_crossing_at_different_elevations_remains_two_wires(){
        let mut scene=Scene::new();let a=scene.mesh.add_vertex(-DVec3::X);let b=scene.mesh.add_vertex(DVec3::X);scene.mesh.add_edge(a,b);scene.geo_paths.push(GeoPath::new(vec![-DVec3::Y+DVec3::Z,DVec3::Y+DVec3::Z],false,"overpass"));geopath_to_geometry(&mut scene,&[0]).unwrap();assert_eq!(scene.mesh.edges.len(),2);assert_eq!(scene.mesh.vertices.len(),4);assert!(scene.mesh.faces.is_empty());
    }
    #[test]fn zero_signed_area_bowtie_detects_two_real_cells(){
        let mut scene=Scene::new();scene.geo_paths.push(GeoPath::new(vec![DVec3::ZERO,DVec3::new(2.,2.,0.),DVec3::new(0.,2.,0.),DVec3::new(2.,0.,0.)],true,"bowtie"));geopath_to_geometry(&mut scene,&[0]).unwrap();assert_eq!(scene.mesh.faces.len(),2);assert_eq!(scene.mesh.edges.len(),6);assert!((scene.mesh.faces.iter().enumerate().map(|(i,_)|scene.mesh.face_area(i)).sum::<f64>()-2.).abs()<1e-9);
    }
    #[test]fn multiple_planes_and_invalid_selected_paths_are_atomic(){
        let mut scene=Scene::new();for z in [0.,2.] {scene.geo_paths.push(GeoPath::new(vec![DVec3::new(0.,0.,z),DVec3::new(1.,0.,z),DVec3::new(1.,1.,z),DVec3::new(0.,1.,z)],true,"footprint"));}geopath_to_geometry(&mut scene,&[0,1]).unwrap();assert_eq!(scene.mesh.faces.len(),2);
        scene.geo_paths.push(GeoPath::new(vec![DVec3::ZERO,DVec3::new(f64::INFINITY,0.,0.)],false,"bad"));let mesh=scene.mesh.clone();let paths=scene.geo_paths.len();assert!(geopath_to_geometry(&mut scene,&[0]).is_err());assert_eq!(scene.mesh,mesh);assert_eq!(scene.geo_paths.len(),paths);assert!(geopath_to_geometry(&mut scene,&[99]).is_err());
    }
}

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub struct TileCapture{
 pub source:serde_json::Value,pub zoom:i64,pub radius_m:f64,pub visible:bool,pub opacity:f64,
 pub patches:Vec<Vec<f64>>,pub url_template:serde_json::Value,pub name:serde_json::Value,pub attribution:serde_json::Value,
}
impl TileCapture{
 pub fn import_reference(previous:Option<&Self>,datum:&SceneDatum,min:DVec3,max:DVec3,requested_zoom:u8,visible:bool)->Self{
  let mut capture=previous.cloned().unwrap_or_else(||Self{
   source:"esri_imagery".into(),zoom:16,radius_m:1200.,visible:true,opacity:1.,
   patches:vec![],url_template:"https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}".into(),
   name:"Esri World Imagery (satellite)".into(),attribution:"Esri, Maxar, Earthstar Geographics, and the GIS community".into(),
  });
  // Source creates a new layer from chosen source/zoom/visibility, resetting
  // radius/opacity while retaining the provider. No imagery is serialized.
  capture.radius_m=1200.;capture.opacity=1.;capture.zoom=i64::from(requested_zoom);capture.visible=visible;
  let width=max.x-min.x;let length=max.y-min.y;
  let margin=0.15*width.max(length).max(200.);
  capture.patches=vec![vec![(min.x+max.x)/2.,(min.y+max.y)/2.,(width+2.*margin)/2.,(length+2.*margin)/2.]];
  while capture.zoom>8&&capture.import_tile_count(datum)>500{capture.zoom-=1;}
  capture
 }
 fn import_tile_count(&self,datum:&SceneDatum)->u64{
  debug_assert_eq!(self.patches.len(),1);
  // Import creates one rectangle. Count its bounded index domain directly;
  // do not allocate millions of tiles merely to choose a coarser zoom.
  self.patches.iter().map(|p|{
   let mut lo_lat=f64::INFINITY;let mut hi_lat=f64::NEG_INFINITY;
   let mut lo_lon=f64::INFINITY;let mut hi_lon=f64::NEG_INFINITY;
   for (x,y) in [(p[0]-p[2],p[1]-p[3]),(p[0]+p[2],p[1]-p[3]),(p[0]+p[2],p[1]+p[3]),(p[0]-p[2],p[1]+p[3])]{
    let(lat,lon,_)=datum.local_to_geodetic(DVec3::new(x,y,0.).as_vec3().as_dvec3());lo_lat=lo_lat.min(lat);hi_lat=hi_lat.max(lat);lo_lon=lo_lon.min(lon);hi_lon=hi_lon.max(lon);
   }
   let(x0,y0)=deg2num(hi_lat,lo_lon,self.zoom as u8);let(x1,y1)=deg2num(lo_lat,hi_lon,self.zoom as u8);let n=(1u64<<self.zoom)as f64;
   let nx=(x0.max(x1).floor().min(n-1.)-x0.min(x1).floor().max(0.)+1.).max(0.)as u64;
   let ny=(y0.max(y1).floor().min(n-1.)-y0.min(y1).floor().max(0.)+1.).max(0.)as u64;
   nx*ny
  }).sum()
 }
}
impl PartialEq for TerrainObject {fn eq(&self,other:&Self)->bool {self.vertices==other.vertices&&self.uvs==other.uvs&&self.triangles==other.triangles&&self.tile_range==other.tile_range&&self.nx==other.nx&&self.ny==other.ny&&self.bbox==other.bbox&&self.visible==other.visible}}


/// A command holds a terrain object; later runtime rebuilds must not become
/// that command's Redo result. Geometry is captured only for actual ownership.
#[derive(Debug,Clone,PartialEq)]
pub struct HeldTerrain {pub object:std::sync::Arc<TerrainObject>,pub visible:bool}
impl HeldTerrain {pub fn capture(t:&TerrainObject)->Self{Self{object:std::sync::Arc::new(t.clone()),visible:t.visible}}pub fn instantiate(&self)->TerrainObject{let mut t=(*self.object).clone();t.visible=self.visible;t}}
#[derive(Debug,Clone,PartialEq)]
pub enum TerrainOwnership {Unowned,Owned{after:Option<HeldTerrain>}}
#[derive(Debug,Clone,Default)]
pub struct TerrainHistory(pub Option<TerrainOwnership>);
impl TerrainHistory {
 pub fn seal(before:&Option<TerrainObject>,after:&Option<TerrainObject>)->TerrainOwnership{if before.as_ref().map(|t|t.history_id)!=after.as_ref().map(|t|t.history_id)||before!=after{TerrainOwnership::Owned{after:after.as_ref().map(HeldTerrain::capture)}}else{TerrainOwnership::Unowned}}
 pub fn refresh_visibility(&mut self,id:uuid::Uuid,visible:bool){if let Some(TerrainOwnership::Owned{after:Some(t)})=&mut self.0{if t.object.history_id==id{t.visible=visible;}}}
}
impl PartialEq for TerrainHistory {fn eq(&self,other:&Self)->bool{match(&self.0,&other.0){(None,Some(TerrainOwnership::Unowned))|(Some(TerrainOwnership::Unowned),None)=>true,(a,b)=>a==b}}}
