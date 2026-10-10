//! Original QColor named palette, frozen from the pinned painter dependency.
pub fn parse_sheet_color(value: &str) -> Result<[u8; 4], String> {
    let text = value.to_ascii_lowercase();
    if let Some(hex) = text.strip_prefix('#') {
        let parse = |s: &str| {
            u16::from_str_radix(s, 16).map_err(|_| format!("Invalid sheet colour: {value}"))
        };
        if !hex.is_ascii() {
            return Err(format!("Invalid sheet colour: {value}"));
        }
        return match hex.len() {
            3 => Ok([
                (parse(&hex[0..1])? * 17) as u8,
                (parse(&hex[1..2])? * 17) as u8,
                (parse(&hex[2..3])? * 17) as u8,
                255,
            ]),
            6 => Ok([
                parse(&hex[0..2])? as u8,
                parse(&hex[2..4])? as u8,
                parse(&hex[4..6])? as u8,
                255,
            ]),
            8 => Ok([
                parse(&hex[2..4])? as u8,
                parse(&hex[4..6])? as u8,
                parse(&hex[6..8])? as u8,
                parse(&hex[0..2])? as u8,
            ]),
            9 | 12 => {
                let digits=hex.len()/3;
                let max=(1_u32 << (digits*4))-1;
                let channel=|part:&str|->Result<u8,String>{Ok(((u32::from(parse(part)?)*255+max/2)/max) as u8)};
                Ok([channel(&hex[..digits])?,channel(&hex[digits..digits*2])?,channel(&hex[digits*2..])?,255])
            },
            _ => Err(format!("Unsupported sheet colour: {value}")),
        };
    }
    match text.as_str() {
        "aliceblue" => Ok([240, 248, 255, 255]),
        "antiquewhite" => Ok([250, 235, 215, 255]),
        "aqua" => Ok([0, 255, 255, 255]),
        "aquamarine" => Ok([127, 255, 212, 255]),
        "azure" => Ok([240, 255, 255, 255]),
        "beige" => Ok([245, 245, 220, 255]),
        "bisque" => Ok([255, 228, 196, 255]),
        "black" => Ok([0, 0, 0, 255]),
        "blanchedalmond" => Ok([255, 235, 205, 255]),
        "blue" => Ok([0, 0, 255, 255]),
        "blueviolet" => Ok([138, 43, 226, 255]),
        "brown" => Ok([165, 42, 42, 255]),
        "burlywood" => Ok([222, 184, 135, 255]),
        "cadetblue" => Ok([95, 158, 160, 255]),
        "chartreuse" => Ok([127, 255, 0, 255]),
        "chocolate" => Ok([210, 105, 30, 255]),
        "coral" => Ok([255, 127, 80, 255]),
        "cornflowerblue" => Ok([100, 149, 237, 255]),
        "cornsilk" => Ok([255, 248, 220, 255]),
        "crimson" => Ok([220, 20, 60, 255]),
        "cyan" => Ok([0, 255, 255, 255]),
        "darkblue" => Ok([0, 0, 139, 255]),
        "darkcyan" => Ok([0, 139, 139, 255]),
        "darkgoldenrod" => Ok([184, 134, 11, 255]),
        "darkgray" => Ok([169, 169, 169, 255]),
        "darkgreen" => Ok([0, 100, 0, 255]),
        "darkgrey" => Ok([169, 169, 169, 255]),
        "darkkhaki" => Ok([189, 183, 107, 255]),
        "darkmagenta" => Ok([139, 0, 139, 255]),
        "darkolivegreen" => Ok([85, 107, 47, 255]),
        "darkorange" => Ok([255, 140, 0, 255]),
        "darkorchid" => Ok([153, 50, 204, 255]),
        "darkred" => Ok([139, 0, 0, 255]),
        "darksalmon" => Ok([233, 150, 122, 255]),
        "darkseagreen" => Ok([143, 188, 143, 255]),
        "darkslateblue" => Ok([72, 61, 139, 255]),
        "darkslategray" => Ok([47, 79, 79, 255]),
        "darkslategrey" => Ok([47, 79, 79, 255]),
        "darkturquoise" => Ok([0, 206, 209, 255]),
        "darkviolet" => Ok([148, 0, 211, 255]),
        "deeppink" => Ok([255, 20, 147, 255]),
        "deepskyblue" => Ok([0, 191, 255, 255]),
        "dimgray" => Ok([105, 105, 105, 255]),
        "dimgrey" => Ok([105, 105, 105, 255]),
        "dodgerblue" => Ok([30, 144, 255, 255]),
        "firebrick" => Ok([178, 34, 34, 255]),
        "floralwhite" => Ok([255, 250, 240, 255]),
        "forestgreen" => Ok([34, 139, 34, 255]),
        "fuchsia" => Ok([255, 0, 255, 255]),
        "gainsboro" => Ok([220, 220, 220, 255]),
        "ghostwhite" => Ok([248, 248, 255, 255]),
        "gold" => Ok([255, 215, 0, 255]),
        "goldenrod" => Ok([218, 165, 32, 255]),
        "gray" => Ok([128, 128, 128, 255]),
        "green" => Ok([0, 128, 0, 255]),
        "greenyellow" => Ok([173, 255, 47, 255]),
        "grey" => Ok([128, 128, 128, 255]),
        "honeydew" => Ok([240, 255, 240, 255]),
        "hotpink" => Ok([255, 105, 180, 255]),
        "indianred" => Ok([205, 92, 92, 255]),
        "indigo" => Ok([75, 0, 130, 255]),
        "ivory" => Ok([255, 255, 240, 255]),
        "khaki" => Ok([240, 230, 140, 255]),
        "lavender" => Ok([230, 230, 250, 255]),
        "lavenderblush" => Ok([255, 240, 245, 255]),
        "lawngreen" => Ok([124, 252, 0, 255]),
        "lemonchiffon" => Ok([255, 250, 205, 255]),
        "lightblue" => Ok([173, 216, 230, 255]),
        "lightcoral" => Ok([240, 128, 128, 255]),
        "lightcyan" => Ok([224, 255, 255, 255]),
        "lightgoldenrodyellow" => Ok([250, 250, 210, 255]),
        "lightgray" => Ok([211, 211, 211, 255]),
        "lightgreen" => Ok([144, 238, 144, 255]),
        "lightgrey" => Ok([211, 211, 211, 255]),
        "lightpink" => Ok([255, 182, 193, 255]),
        "lightsalmon" => Ok([255, 160, 122, 255]),
        "lightseagreen" => Ok([32, 178, 170, 255]),
        "lightskyblue" => Ok([135, 206, 250, 255]),
        "lightslategray" => Ok([119, 136, 153, 255]),
        "lightslategrey" => Ok([119, 136, 153, 255]),
        "lightsteelblue" => Ok([176, 196, 222, 255]),
        "lightyellow" => Ok([255, 255, 224, 255]),
        "lime" => Ok([0, 255, 0, 255]),
        "limegreen" => Ok([50, 205, 50, 255]),
        "linen" => Ok([250, 240, 230, 255]),
        "magenta" => Ok([255, 0, 255, 255]),
        "maroon" => Ok([128, 0, 0, 255]),
        "mediumaquamarine" => Ok([102, 205, 170, 255]),
        "mediumblue" => Ok([0, 0, 205, 255]),
        "mediumorchid" => Ok([186, 85, 211, 255]),
        "mediumpurple" => Ok([147, 112, 219, 255]),
        "mediumseagreen" => Ok([60, 179, 113, 255]),
        "mediumslateblue" => Ok([123, 104, 238, 255]),
        "mediumspringgreen" => Ok([0, 250, 154, 255]),
        "mediumturquoise" => Ok([72, 209, 204, 255]),
        "mediumvioletred" => Ok([199, 21, 133, 255]),
        "midnightblue" => Ok([25, 25, 112, 255]),
        "mintcream" => Ok([245, 255, 250, 255]),
        "mistyrose" => Ok([255, 228, 225, 255]),
        "moccasin" => Ok([255, 228, 181, 255]),
        "navajowhite" => Ok([255, 222, 173, 255]),
        "navy" => Ok([0, 0, 128, 255]),
        "oldlace" => Ok([253, 245, 230, 255]),
        "olive" => Ok([128, 128, 0, 255]),
        "olivedrab" => Ok([107, 142, 35, 255]),
        "orange" => Ok([255, 165, 0, 255]),
        "orangered" => Ok([255, 69, 0, 255]),
        "orchid" => Ok([218, 112, 214, 255]),
        "palegoldenrod" => Ok([238, 232, 170, 255]),
        "palegreen" => Ok([152, 251, 152, 255]),
        "paleturquoise" => Ok([175, 238, 238, 255]),
        "palevioletred" => Ok([219, 112, 147, 255]),
        "papayawhip" => Ok([255, 239, 213, 255]),
        "peachpuff" => Ok([255, 218, 185, 255]),
        "peru" => Ok([205, 133, 63, 255]),
        "pink" => Ok([255, 192, 203, 255]),
        "plum" => Ok([221, 160, 221, 255]),
        "powderblue" => Ok([176, 224, 230, 255]),
        "purple" => Ok([128, 0, 128, 255]),
        "red" => Ok([255, 0, 0, 255]),
        "rosybrown" => Ok([188, 143, 143, 255]),
        "royalblue" => Ok([65, 105, 225, 255]),
        "saddlebrown" => Ok([139, 69, 19, 255]),
        "salmon" => Ok([250, 128, 114, 255]),
        "sandybrown" => Ok([244, 164, 96, 255]),
        "seagreen" => Ok([46, 139, 87, 255]),
        "seashell" => Ok([255, 245, 238, 255]),
        "sienna" => Ok([160, 82, 45, 255]),
        "silver" => Ok([192, 192, 192, 255]),
        "skyblue" => Ok([135, 206, 235, 255]),
        "slateblue" => Ok([106, 90, 205, 255]),
        "slategray" => Ok([112, 128, 144, 255]),
        "slategrey" => Ok([112, 128, 144, 255]),
        "snow" => Ok([255, 250, 250, 255]),
        "springgreen" => Ok([0, 255, 127, 255]),
        "steelblue" => Ok([70, 130, 180, 255]),
        "tan" => Ok([210, 180, 140, 255]),
        "teal" => Ok([0, 128, 128, 255]),
        "thistle" => Ok([216, 191, 216, 255]),
        "tomato" => Ok([255, 99, 71, 255]),
        "transparent" => Ok([0, 0, 0, 0]),
        "turquoise" => Ok([64, 224, 208, 255]),
        "violet" => Ok([238, 130, 238, 255]),
        "wheat" => Ok([245, 222, 179, 255]),
        "white" => Ok([255, 255, 255, 255]),
        "whitesmoke" => Ok([245, 245, 245, 255]),
        "yellow" => Ok([255, 255, 0, 255]),
        "yellowgreen" => Ok([154, 205, 50, 255]),
        _ => Err(format!("Unsupported sheet colour: {value}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_long_hex_all_channel_values() {
        let expected=include_bytes!("../../../docs/audits/sheet-shape-original/original-long-hex-rgba.bin");
        let mut index=0;
        for digits in [3,4] {
            for value in 0..(1_u32 << (digits*4)) {
                let channel=format!("{value:0digits$x}");
                let text=format!("#{channel}{channel}{channel}");
                assert_eq!(parse_sheet_color(&text).unwrap(),expected[index..index+4],"{text}");index+=4;
            }
        }
        assert_eq!(index,expected.len());
        assert_eq!(parse_sheet_color("#123456789").unwrap(),[18,69,120,255]);
        assert_eq!(parse_sheet_color("#123456789abc").unwrap(),[18,86,154,255]);
    }
    #[test]
    fn original_named_palette_and_hex_channel_order() {
        let names: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/sheet-shape-original/original-named-colors.json"
        ))
        .unwrap();
        for (name, expected) in names.as_object().unwrap() {
            assert_eq!(
                serde_json::to_value(parse_sheet_color(name).unwrap()).unwrap(),
                *expected
            );
        }
        assert_eq!(
            parse_sheet_color("DaRkSlAtEbLuE").unwrap(),
            [72, 61, 139, 255]
        );
        assert_eq!(parse_sheet_color("#abc").unwrap(), [170, 187, 204, 255]);
        assert_eq!(parse_sheet_color("#80112233").unwrap(), [17, 34, 51, 128]);
        assert_eq!(parse_sheet_color("transparent").unwrap(), [0, 0, 0, 0]);
        for value in ["#１２３", "#ggg", "#ff", "not-a-colour"] {
            assert!(parse_sheet_color(value).is_err());
        }
    }
}
