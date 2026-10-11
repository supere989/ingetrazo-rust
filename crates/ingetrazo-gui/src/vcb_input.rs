//! Original model viewport keyboard VCB ownership; composer/navigation retain their editors.
use crate::app::IngeTrazoApp;
use ingetrazo_core::units::vcb;
use ingetrazo_tools::base::ToolType;

fn chord(m: egui::Modifiers) -> bool {
    (m.ctrl || m.alt || m.mac_cmd) && !(m.ctrl && m.alt)
}
fn digit(c: char) -> bool {
    let mut bytes = [0; 4];
    let n = vcb::normalize_source_numeric(c.encode_utf8(&mut bytes));
    (n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
        || EXTRA_DIGITS.binary_search(&(c as u32)).is_ok()
}
// Python 3.13 / UCD 15.1 Digit-only characters; decimal characters use the
// existing pinned normalizer. Numeric-only fractions/Roman numerals are excluded.
const EXTRA_DIGITS: &[u32] = &[
    178, 179, 185, 4969, 4970, 4971, 4972, 4973, 4974, 4975, 4976, 4977, 6618, 8304, 8308, 8309,
    8310, 8311, 8312, 8313, 8320, 8321, 8322, 8323, 8324, 8325, 8326, 8327, 8328, 8329, 9312, 9313,
    9314, 9315, 9316, 9317, 9318, 9319, 9320, 9332, 9333, 9334, 9335, 9336, 9337, 9338, 9339, 9340,
    9352, 9353, 9354, 9355, 9356, 9357, 9358, 9359, 9360, 9450, 9461, 9462, 9463, 9464, 9465, 9466,
    9467, 9468, 9469, 9471, 10102, 10103, 10104, 10105, 10106, 10107, 10108, 10109, 10110, 10112,
    10113, 10114, 10115, 10116, 10117, 10118, 10119, 10120, 10122, 10123, 10124, 10125, 10126,
    10127, 10128, 10129, 10130, 68160, 68161, 68162, 68163, 69216, 69217, 69218, 69219, 69220,
    69221, 69222, 69223, 69224, 69714, 69715, 69716, 69717, 69718, 69719, 69720, 69721, 69722,
    127232, 127233, 127234, 127235, 127236, 127237, 127238, 127239, 127240, 127241, 127242,
];
fn append(buffer: &mut String, c: char, arrays: bool) -> bool {
    let tail = buffer.rsplit([';', ' ']).next().unwrap_or("");
    let lower = c.to_ascii_lowercase();
    if arrays && (c == '*' || lower == 'x' && !tail.is_empty() || c == '/' && buffer.is_empty()) {
        buffer.push(c);
        return true;
    }
    let unit = "mcrinft".contains(lower);
    if !digit(c) && !r#".,; -:"'/"#.contains(c) && !unit {
        return false;
    }
    if (c == ';' || c == ' ') && buffer.is_empty() || unit && tail.is_empty() {
        return false;
    }
    if r#""'/"#.contains(c) && tail.is_empty()
        || c == '-' && !tail.is_empty()
        || c == ':' && (tail.is_empty() || buffer.contains(':'))
        || (c == '.' || c == ',') && (tail.contains('.') || tail.contains(','))
    {
        return true;
    }
    buffer.push(c);
    true
}
fn decimal(t: &str) -> bool {
    if let Some((a, b)) = t.split_once('.') {
        (!a.is_empty() || !b.is_empty())
            && a.bytes().all(|c| c.is_ascii_digit())
            && b.bytes().all(|c| c.is_ascii_digit())
    } else {
        !t.is_empty() && t.bytes().all(|c| c.is_ascii_digit())
    }
}
fn array(t: &str) -> bool {
    let digits = |s: &str| !s.trim().is_empty() && s.trim().bytes().all(|c| c.is_ascii_digit());
    if let Some(n) = t.strip_prefix('/') {
        return digits(n);
    }
    if let Some(n) = t.strip_suffix('/') {
        return digits(n);
    }
    if let Some(n) = t.strip_prefix(['*', 'x', 'X']) {
        return digits(n);
    }
    if let Some((n, rest)) = t.split_once(['*', 'x', 'X']) {
        return digits(n)
            && (rest.trim().is_empty() || vcb::parse_single(rest, 1.).is_ok_and(|v| v > 0.));
    }
    false
}
fn valid(text: &str, tool: ToolType) -> bool {
    let text = vcb::normalize_source_numeric(text);
    let t = text.trim();
    if t.contains(':') {
        return matches!(tool, ToolType::Rotate | ToolType::Protractor)
            && vcb::parse_angle(t).is_ok();
    }
    if let Some(radius) = t.strip_suffix(['r', 'R']) {
        return matches!(tool, ToolType::Arc) && decimal(radius);
    }
    if t.contains('*') || t.contains('x') || t.contains('X') || t.starts_with('/') {
        return matches!(tool, ToolType::Move | ToolType::Rotate) && array(t);
    }
    vcb::parse_values(t, 1., tool == ToolType::Rectangle).is_ok()
}
pub(crate) fn plain_key(i: &egui::InputState, key: egui::Key) -> bool {
    i.events.iter().any(|e|matches!(e,egui::Event::Key{key:k,pressed:true,modifiers,..} if *k==key&&!(modifiers.ctrl||modifiers.alt||modifiers.mac_cmd)))
}
pub(crate) fn route(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    if app.scene.active_sheet_index.is_some() || app.navigation.mode.is_some() {
        return;
    }
    let focus = ctx.memory(|m| m.focused());
    if ctx.wants_keyboard_input() && focus != Some(egui::Id::new("measurements_vcb")) {
        return;
    }
    if !ctx.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Text(_) | egui::Event::Key { pressed: true, .. }
            )
        })
    }) {
        return;
    }
    let (mut events, mut modifiers) =
        ctx.input_mut(|i| (std::mem::take(&mut i.events), i.modifiers));
    if events.is_empty() {
        return;
    }
    let mut remove = vec![false; events.len()];
    let mut pending = None;
    for (index, event) in events.iter().enumerate() {
        match event {
            egui::Event::Key {
                key,
                pressed: true,
                modifiers: m,
                ..
            } => {
                modifiers = *m;
                pending = Some(index);
                match key {
                    egui::Key::Enter if !app.vcb_text.is_empty() => {
                        let text = std::mem::take(&mut app.vcb_text);
                        if valid(&text, app.active_tool_type) {
                            if let Err(e) = app.dispatch_tool_vcb(&text) {
                                app.status_message = e;
                            } else {
                                app.status_message.clear();
                            }
                        }
                        remove[index] = true;
                        pending = None;
                    }
                    egui::Key::Backspace if !app.vcb_text.is_empty() => {
                        app.vcb_text.pop();
                        remove[index] = true;
                        pending = None;
                    }
                    egui::Key::Escape if !app.vcb_text.is_empty() => {
                        app.vcb_text.clear();
                        remove[index] = true;
                        pending = None;
                    }
                    egui::Key::R if !chord(*m) && m.shift && app.vcb_text.is_empty() => {
                        app.repeat_last_command();
                        remove[index] = true;
                        pending = None;
                    }
                    _ if app.vcb_text.is_empty() && !(m.ctrl || m.alt || m.mac_cmd) => {
                        let tool = match key {
                            egui::Key::Space => Some(ToolType::Select),
                            egui::Key::L => Some(ToolType::Line),
                            egui::Key::R => Some(ToolType::Rectangle),
                            egui::Key::M => Some(ToolType::Move),
                            egui::Key::Q => Some(ToolType::Rotate),
                            egui::Key::S => Some(ToolType::Scale),
                            egui::Key::T => Some(ToolType::Tape),
                            egui::Key::E => Some(ToolType::Eraser),
                            egui::Key::B => Some(ToolType::Paint),
                            egui::Key::P => Some(ToolType::PushPull),
                            _ => None,
                        };
                        if let Some(tool) = tool {
                            app.set_tool(tool);
                            remove[index] = true;
                            pending = None;
                        }
                    }
                    _ => {}
                }
            }
            egui::Event::Text(text) if !chord(modifiers) => {
                if text.is_empty() || !(text.chars().all(digit) || text.chars().count() == 1) {
                    continue;
                }
                let arrays = matches!(app.active_tool_type, ToolType::Move | ToolType::Rotate);
                let mut handled = false;
                for c in text.chars() {
                    handled |= append(&mut app.vcb_text, c, arrays);
                }
                if handled {
                    remove[index] = true;
                    if let Some(k) = pending.take() {
                        remove[k] = true;
                    }
                }
            }
            _ => {}
        }
    }
    let mut index = 0;
    events.retain(|_| {
        let keep = !remove[index];
        index += 1;
        keep
    });
    ctx.input_mut(|i| i.events = events);
}
