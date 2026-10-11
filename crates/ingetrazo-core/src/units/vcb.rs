//! Source VCB length fields. Parsing happens once per submitted entry.

/// Python3.13/UCD15.1 numeric ingress accepts Unicode decimal digits, not
/// all Unicode numeric symbols. Keep ordinary ASCII submission allocation-free.
/// The source also treats ASCII information separators as whitespace.
pub fn normalize_source_numeric(text:&str)->std::borrow::Cow<'_,str> {
    if text.bytes().all(|b|b.is_ascii()&& !(0x1c..=0x1f).contains(&b)){return std::borrow::Cow::Borrowed(text);}
    const ZERO:&[u32]=&[0x30,0x660,0x6f0,0x7c0,0x966,0x9e6,0xa66,0xae6,0xb66,0xbe6,0xc66,0xce6,0xd66,0xde6,0xe50,0xed0,0xf20,0x1040,0x1090,0x17e0,0x1810,0x1946,0x19d0,0x1a80,0x1a90,0x1b50,0x1bb0,0x1c40,0x1c50,0xa620,0xa8d0,0xa900,0xa9d0,0xa9f0,0xaa50,0xabf0,0xff10,0x104a0,0x10d30,0x11066,0x110f0,0x11136,0x111d0,0x112f0,0x11450,0x114d0,0x11650,0x116c0,0x11730,0x118e0,0x11950,0x11c50,0x11d50,0x11da0,0x11f50,0x16a60,0x16ac0,0x16b50,0x1d7ce,0x1d7d8,0x1d7e2,0x1d7ec,0x1d7f6,0x1e140,0x1e2f0,0x1e4f0,0x1e950,0x1fbf0];
    std::borrow::Cow::Owned(text.chars().map(|c|{
        let n=c as u32;
        if (0x1c..=0x1f).contains(&n){return ' ';}
        if let Some(&zero)=ZERO.iter().find(|&&zero|n>=zero&&n<zero+10){return char::from_u32(48+n-zero).unwrap();}
        c
    }).collect())
}

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
    let text=normalize_source_numeric(text);let text=text.as_ref();
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
    let text=normalize_source_numeric(text);let text=text.as_ref();
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
