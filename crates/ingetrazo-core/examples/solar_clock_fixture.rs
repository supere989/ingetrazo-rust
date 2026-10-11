use ingetrazo_core::{
    georef::SceneDatum,
    presentation::ShadowSettings,
    sun::{self, DateTime},
};
use std::io::{self, Read};
fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let rows: serde_json::Value = serde_json::from_str(&input).unwrap();
    let mut out = Vec::new();
    for row in rows.as_array().unwrap() {
        let lat = row["lat"].as_f64().unwrap();
        let lon = row["lon"].as_f64().unwrap();
        let date: DateTime = serde_json::from_value(row["utc"].clone()).unwrap();
        let settings = ShadowSettings::try_from_json(&row["settings"]).unwrap();
        let year = row["year"].as_i64().unwrap() as i32;
        let local = sun::when_utc(&settings, lon, year);
        let datum = row["north"]
            .as_f64()
            .map(|north| SceneDatum::new(lat, lon, 0., north));
        let scene = sun::scene_direction(&settings, datum.as_ref(), year);
        let daylight = if settings.source_utc_offset.is_some() {
            Err("Solar UTC offset is outside the clock domain".into())
        } else {
            sun::daylight_minutes(
                lat,
                lon,
                settings.month,
                settings.day,
                settings.utc_offset,
                year,
                5,
            )
        };
        out.push(serde_json::json!({"scene_direction":scene.as_ref().ok(),"invalid_scene":scene.is_err(),"position":sun::solar_position(lat,lon,date).unwrap(),"direction":sun::sun_direction(lat,lon,date).unwrap(),"offset":sun::default_utc_offset(lon).unwrap(),"utc":local.as_ref().ok(),"invalid_local":local.is_err(),"daylight":daylight.as_ref().ok().copied().flatten(),"invalid_daylight":daylight.is_err()}));
    }
    println!("{}", serde_json::to_string(&out).unwrap());
}
