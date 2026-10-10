fn fraction_inches(inches: f64, n: i32) -> Result<String, String> {
    let denominator = match n.clamp(0, 4) {
        0 => 1.,
        1 => 4.,
        2 => 16.,
        3 => 32.,
        _ => 64.,
    };
    let scaled = inches * denominator;
    if !scaled.is_finite() {
        return Err("Annotation fractional conversion overflow".into());
    }
    let count = scaled.round_ties_even();
    let whole = (count / denominator).trunc();
    let numerator = count % denominator;
    if numerator == 0. {
        return Ok(format!("{whole:.0}"));
    }
    let mut a = numerator as u32;
    let mut b = denominator as u32;
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    let rest = format!("{}/{}", numerator as u32 / a, denominator as u32 / a);
    Ok(if whole == 0. {
        rest
    } else {
        format!("{whole:.0} {rest}")
    })
}
pub fn format_length(meters: f64, units: &str, decimals: i32) -> Result<String, String> {
    if !meters.is_finite() {
        return Err("Nonfinite annotation measured length".into());
    }
    let n = decimals.clamp(0, 6) as usize;
    let u = if units.is_empty() { "m" } else { units };
    Ok(match u {
        "in" => {
            let value = meters / 0.0254;
            if !value.is_finite() {
                return Err("Annotation unit conversion overflow".into());
            }
            format!("{value:.n$}\"")
        }
        "ft" => {
            let value = meters / 0.3048;
            if !value.is_finite() {
                return Err("Annotation unit conversion overflow".into());
            }
            format!("{value:.n$}'")
        }
        "ft-in" | "ft-in-frac" | "in-frac" => {
            let sign = if meters < 0. { "-" } else { "" };
            let total = meters.abs() / 0.0254;
            if !total.is_finite() {
                return Err("Annotation unit conversion overflow".into());
            }
            if u == "in-frac" {
                format!("{sign}{}\"", fraction_inches(total, n as i32)?)
            } else {
                let mut feet = (total / 12.).floor();
                let mut inches = total - feet * 12.;
                if u == "ft-in" {
                    if format!("{inches:.n$}").parse::<f64>().unwrap_or(inches) >= 12. {
                        feet += 1.;
                        inches = 0.;
                    }
                    format!("{sign}{feet:.0}'{inches:.n$}\"")
                } else {
                    let mut text = fraction_inches(inches, n as i32)?;
                    if text == "12" {
                        feet += 1.;
                        text = "0".into();
                    }
                    format!("{sign}{feet:.0}'{text}\"")
                }
            }
        }
        _ => {
            let factor = match u {
                "cm" => 100.,
                "mm" => 1000.,
                _ => 1.,
            };
            let suffix = if matches!(u, "m" | "cm" | "mm") {
                u
            } else {
                "m"
            };
            let converted = meters * factor;
            if !converted.is_finite() {
                return Err("Annotation unit conversion overflow".into());
            }
            format!("{converted:.n$} {suffix}")
        }
    })
}

fn main(){use std::io::BufRead;for line in std::io::stdin().lock().lines(){let l=line.unwrap();let a:Vec<_>=l.split_whitespace().collect();let v=f64::from_bits(a[0].parse().unwrap());match format_length(v,a[1],a[2].parse().unwrap()){Ok(s)=>println!("{}",s),Err(e)=>println!("ERR:{}",e)}}}
