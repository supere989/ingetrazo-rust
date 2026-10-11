//! Solar clock and geographic direction from the pinned original core/sun.py.
//! Rendering and system-clock selection remain callers' responsibilities.
use crate::{georef::SceneDatum, presentation::ShadowSettings};
use serde::{Deserialize, Serialize};
pub const DEFAULT_LAT: f64 = -16.409;
pub const DEFAULT_LON: f64 = -71.537;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Position {
    pub elevation: f64,
    pub azimuth: f64,
}
fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}
impl DateTime {
    pub fn validate(self) -> Result<Self, String> {
        if !(1..=9999).contains(&self.year)
            || self.day == 0
            || self.day > days_in_month(self.year, self.month)
            || self.hour > 23
            || self.minute > 59
            || self.second > 59
        {
            return Err("Solar date/time is invalid".into());
        }
        Ok(self)
    }
    fn shift_day(&mut self, forward: bool) -> Result<(), String> {
        if forward {
            if self.day < days_in_month(self.year, self.month) {
                self.day += 1;
            } else {
                self.day = 1;
                if self.month < 12 {
                    self.month += 1;
                } else {
                    self.year += 1;
                    self.month = 1;
                }
            }
        } else if self.day > 1 {
            self.day -= 1;
        } else {
            if self.month > 1 {
                self.month -= 1;
            } else {
                self.year -= 1;
                self.month = 12;
            }
            self.day = days_in_month(self.year, self.month);
        }
        self.validate().map(|_| ())
    }
}
pub fn default_utc_offset(lon: f64) -> Result<i32, String> {
    let value = (lon / 15.).round_ties_even();
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err("Solar longitude cannot define a clock offset".into());
    }
    Ok(value as i32)
}
pub fn when_utc(settings: &ShadowSettings, lon: f64, year: i32) -> Result<DateTime, String> {
    if settings.source_utc_offset.is_some() {
        return Err("Solar UTC offset is outside the clock domain".into());
    }
    let offset = match settings.utc_offset {
        Some(v) => v,
        None => default_utc_offset(lon)?,
    };
    if !(-23..=23).contains(&offset) {
        return Err("Solar UTC offset must be strictly between minus 24 and 24 hours".into());
    }
    let mut date = DateTime {
        year,
        month: settings.month,
        day: settings.day,
        hour: settings.hour,
        minute: settings.minute,
        second: 0,
    }
    .validate()?;
    let minutes = i32::from(date.hour) * 60 + i32::from(date.minute) - offset * 60;
    if minutes < 0 {
        date.shift_day(false)?;
    } else if minutes >= 1440 {
        date.shift_day(true)?;
    }
    let minutes = minutes.rem_euclid(1440);
    date.hour = (minutes / 60) as u8;
    date.minute = (minutes % 60) as u8;
    Ok(date)
}
pub fn solar_position(lat: f64, lon: f64, date: DateTime) -> Result<Position, String> {
    date.validate()?;
    if !lat.is_finite() || !lon.is_finite() {
        return Err("Solar location must be finite".into());
    }
    let (mut year, mut month) = (date.year, i32::from(date.month));
    let day = f64::from(date.day)
        + f64::from(date.hour) / 24.
        + f64::from(date.minute) / 1440.
        + f64::from(date.second) / 86400.;
    if month <= 2 {
        year -= 1;
        month += 12;
    }
    let a = year.div_euclid(100);
    let b = 2 - a + a.div_euclid(4);
    let jd = (365.25 * f64::from(year + 4716)).floor()
        + (30.6001 * f64::from(month + 1)).floor()
        + day
        + f64::from(b)
        - 1524.5;
    let t = (jd - 2451545.) / 36525.;
    let l0 = (280.46646 + t * (36000.76983 + 0.0003032 * t)).rem_euclid(360.);
    let anomaly = 357.52911 + t * (35999.05029 - 0.0001537 * t);
    let ecc = 0.016708634 - t * (0.000042037 + 0.0000001267 * t);
    let mr = anomaly.to_radians();
    let center = mr.sin() * (1.914602 - t * (0.004817 + 0.000014 * t))
        + (2. * mr).sin() * (0.019993 - 0.000101 * t)
        + (3. * mr).sin() * 0.000289;
    let true_long = l0 + center;
    let omega = 125.04 - 1934.136 * t;
    let app_long = true_long - 0.00569 - 0.00478 * omega.to_radians().sin();
    let obliq0 = 23. + (26. + (21.448 - t * (46.815 + t * (0.00059 - t * 0.001813))) / 60.) / 60.;
    let obliq = obliq0 + 0.00256 * omega.to_radians().cos();
    let decl = (obliq.to_radians().sin() * app_long.to_radians().sin())
        .asin()
        .to_degrees();
    let var_y = (obliq / 2.).to_radians().tan().powi(2);
    let l0r = l0.to_radians();
    let eot = 4.
        * (var_y * (2. * l0r).sin() - 2. * ecc * mr.sin()
            + 4. * ecc * var_y * mr.sin() * (2. * l0r).cos()
            - 0.5 * var_y * var_y * (4. * l0r).sin()
            - 1.25 * ecc * ecc * (2. * mr).sin())
        .to_degrees();
    let minutes =
        f64::from(date.hour) * 60. + f64::from(date.minute) + f64::from(date.second) / 60.;
    let tst = (minutes + eot + 4. * lon).rem_euclid(1440.);
    let mut ha = tst / 4. - 180.;
    if ha < -180. {
        ha += 360.;
    }
    let latr = lat.to_radians();
    let declr = decl.to_radians();
    let har = ha.to_radians();
    let cos_zen = (latr.sin() * declr.sin() + latr.cos() * declr.cos() * har.cos()).clamp(-1., 1.);
    let zenith = cos_zen.acos().to_degrees();
    let elevation = 90. - zenith;
    let denom = latr.cos() * zenith.to_radians().sin();
    let azimuth = if denom.abs() < 1e-12 {
        180.
    } else {
        let az = ((latr.sin() * cos_zen - declr.sin()) / denom)
            .clamp(-1., 1.)
            .acos()
            .to_degrees();
        if ha > 0. {
            (az + 180.).rem_euclid(360.)
        } else {
            (540. - az).rem_euclid(360.)
        }
    };
    Ok(Position { elevation, azimuth })
}
pub fn sun_direction(lat: f64, lon: f64, date: DateTime) -> Result<Option<[f64; 3]>, String> {
    let p = solar_position(lat, lon, date)?;
    if p.elevation <= 0. {
        return Ok(None);
    }
    let e = p.elevation.to_radians();
    let a = p.azimuth.to_radians();
    Ok(Some([e.cos() * a.sin(), e.cos() * a.cos(), e.sin()]))
}
pub fn scene_direction(
    settings: &ShadowSettings,
    datum: Option<&SceneDatum>,
    year: i32,
) -> Result<Option<[f64; 3]>, String> {
    if !settings.enabled {
        return Ok(None);
    }
    let (lat, lon) = datum.map_or((DEFAULT_LAT, DEFAULT_LON), |d| (d.lat, d.lon));
    let Some(mut dir) = sun_direction(lat, lon, when_utc(settings, lon, year)?)? else {
        return Ok(None);
    };
    if let Some(d) = datum.filter(|d| d.north != 0.) {
        let (x, y) = d.grid_to_local_xy(dir[0], dir[1]);
        dir[0] = x;
        dir[1] = y;
    }
    Ok(Some(dir))
}
pub fn daylight_minutes(
    lat: f64,
    lon: f64,
    month: u8,
    day: u8,
    offset: Option<i32>,
    year: i32,
    step: u16,
) -> Result<Option<(u16, u16)>, String> {
    if step == 0 {
        return Err("Solar daylight step must be positive".into());
    }
    let mut first = None;
    let mut last = 0;
    for minute in (0..1440u16).step_by(usize::from(step)) {
        let settings = ShadowSettings {
            month,
            day,
            hour: (minute / 60) as u8,
            minute: (minute % 60) as u8,
            utc_offset: offset,
            ..Default::default()
        };
        if solar_position(lat, lon, when_utc(&settings, lon, year)?)?.elevation > 0. {
            first.get_or_insert(minute);
            last = minute;
        }
    }
    Ok(first.map(|first| {
        (
            first,
            (u32::from(last) + u32::from(step) - 1).min(1439) as u16,
        )
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_clock_crosses_year_and_leap_day() {
        let mut s = ShadowSettings {
            month: 1,
            day: 1,
            hour: 0,
            utc_offset: Some(1),
            ..Default::default()
        };
        assert_eq!(
            when_utc(&s, 0., 2026).unwrap(),
            DateTime {
                year: 2025,
                month: 12,
                day: 31,
                hour: 23,
                minute: 0,
                second: 0
            }
        );
        s.month = 2;
        s.day = 29;
        assert!(when_utc(&s, 0., 2026).is_err());
        assert_eq!(when_utc(&s, 0., 2024).unwrap().day, 28);
        s.month = 12;
        s.day = 31;
        s.hour = 23;
        s.utc_offset = Some(-1);
        assert_eq!(when_utc(&s, 0., 2026).unwrap().year, 2027);
    }
    #[test]
    fn timezone_uses_source_half_even_and_rejects_invalid_offsets() {
        for (lon, expected) in [
            (-37.5, -2),
            (-22.5, -2),
            (-7.5, 0),
            (7.5, 0),
            (22.5, 2),
            (37.5, 2),
        ] {
            assert_eq!(default_utc_offset(lon).unwrap(), expected);
        }
        assert!(when_utc(
            &ShadowSettings {
                utc_offset: Some(24),
                ..Default::default()
            },
            0.,
            2026
        )
        .is_err());
    }
}
