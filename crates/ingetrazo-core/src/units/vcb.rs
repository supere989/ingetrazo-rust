//! Source VCB length fields. Parsing happens once per submitted entry.

fn digits(s: &str) -> bool { !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) }
fn decimal(s: &str, leading_dot: bool) -> bool {
    if let Some((a,b))=s.split_once('.') {
        (digits(a) || (leading_dot && a.is_empty())) && (b.is_empty() || digits(b))
            && !(a.is_empty() && b.is_empty())
    } else { digits(s) }
}
fn number(s: &str) -> Option<f64> {
    let (whole,tail)=if let Some((a,b))=s.split_once('-') {
        if !digits(a) { return None; }
        (Some(a.parse::<f64>().ok()?),b)
    } else { (None,s) };
    let mut value=if let Some((a,b))=tail.split_once('/') {
        if !decimal(a,true)||!decimal(b,false) { return None; }
        let denominator=b.parse::<f64>().ok()?;
        if denominator==0. { return None; }
        a.parse::<f64>().ok()?/denominator
    } else {
        if whole.is_some()||!decimal(tail,true) { return None; }
        tail.parse::<f64>().ok()?
    };
    value+=whole.unwrap_or(0.);
    (value.is_finite()&&value.abs()<=1e9).then_some(value)
}
fn suffix(s: &str)->(&str,Option<f64>) {
    for (mark,scale) in [("mm",0.001),("cm",0.01),("m",1.),("in",0.0254),("ft",0.3048),("\"",0.0254),("'",0.3048)] {
        if let Some(rest)=s.strip_suffix(mark) { return (rest,Some(scale)); }
    }
    (s,None)
}
fn field(s: &str,scale:f64)->Option<f64> {
    let (sign,s)=s.strip_prefix('-').map_or((1.,s),|s|(-1.,s));
    if let Some((feet,inches))=s.split_once('\'') {
        let inches=inches.strip_suffix('"').unwrap_or(inches);
        if !inches.is_empty() {
            return Some(sign*(number(feet)?*0.3048+number(inches)?*0.0254));
        }
    }
    let (num,explicit)=suffix(s);
    Some(sign*number(num)?*explicit.unwrap_or(scale))
}
fn mixed_head(s: &str)->bool {
    let s=s.strip_prefix('-').unwrap_or(s);
    if let Some((a,b))=s.split_once('\'') { digits(a)&&(b.is_empty()||digits(b)) }else{digits(s)}
}
fn fraction_field(s: &str)->bool {
    let (s,_)=suffix(s);
    s.split_once('/').is_some_and(|(a,b)|digits(a)&&digits(b))
}

/// Source comma policy: only a multi-value tool with no spaces or semicolon
/// uses commas as separators. Otherwise a comma is decimal punctuation.
/// Bare numbers use `scale`; explicit units retain their own conversion.
pub fn parse_values(text:&str,scale:f64,comma_lists:bool)->Result<Vec<f64>,String> {
    let lists=comma_lists&&!text.contains(';')&&!text.trim().chars().any(char::is_whitespace);
    let normalized=text.to_ascii_lowercase().replace(',',if lists {" "}else{"."}).replace(';'," ");
    let fields:Vec<_>=normalized.split_whitespace().collect();
    let mut values=Vec::with_capacity(fields.len().min(3));
    let mut i=0;
    while i<fields.len() {
        let value=if i+1<fields.len()&&mixed_head(fields[i])&&fraction_field(fields[i+1]) {
            let joint=if fields[i].ends_with('\''){""}else{"-"};
            let merged=format!("{}{joint}{}",fields[i],fields[i+1]);i+=2;field(&merged,scale)
        }else{let v=field(fields[i],scale);i+=1;v};
        values.push(value.ok_or("Invalid measurement: enter finite coordinates or lengths")?);
        if values.len()>3 { return Err("Enter one, two or three measurements".into()); }
    }
    if values.is_empty(){return Err("Empty measurement".into());}
    Ok(values)
}
pub fn parse_single(text:&str,scale:f64)->Result<f64,String> {
    let values=parse_values(text,scale,false)?;
    if values.len()!=1{return Err("Enter one measurement".into());}
    Ok(values[0])
}
/// Angle tools accept rise:run as degrees. Bare values are unitless.
pub fn parse_angle(text:&str)->Result<f64,String> {
    if let Some((rise,run))=text.trim().split_once(':') {
        let rise=rise.replace(',',".");let run=run.replace(',',".");
        let r=rise.strip_prefix('-').unwrap_or(&rise);
        if !decimal(r,true)||!decimal(&run,true) { return Err("Invalid rise:run angle".into()); }
        let rise: f64=rise.parse().map_err(|_|"Invalid rise")?;
        let run:f64=run.parse().map_err(|_|"Invalid run")?;
        if !rise.is_finite()||!run.is_finite()||run<=0. { return Err("Angle run must be finite and positive".into()); }
        return Ok(rise.atan2(run).to_degrees());
    }
    parse_single(text,1.)
}

/// Valid parsed fields carrying an explicit unit make Scale absolute.
pub fn has_explicit_unit(text:&str)->bool {
    text.to_ascii_lowercase().split(|c:char|c.is_whitespace()||c==';').any(|field|field.contains(['\'','"'])||suffix(field).1.is_some())
}
