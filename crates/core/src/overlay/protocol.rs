use crate::action::{MouseButton, Point};
use crate::node::Rect;

/// Key overlay payload constrained to text or combo, never both.
pub enum KeyEvent {
    Text(String),
    Combo(String),
}

/// Formats a cursor move event.
pub fn format_move(x: f64, y: f64, target_pid: Option<i32>) -> String {
    format!(
        r#"{{"kind":"move","x":{},"y":{}{}}}"#,
        fmt_num(x),
        fmt_num(y),
        target_pid_field(target_pid)
    )
}

/// Formats a cursor click event.
pub fn format_click(
    x: f64,
    y: f64,
    button: &MouseButton,
    count: u32,
    target_pid: Option<i32>,
) -> String {
    format!(
        r#"{{"kind":"click","x":{},"y":{},"button":"{}","count":{}{}}}"#,
        fmt_num(x),
        fmt_num(y),
        button_str(button),
        count,
        target_pid_field(target_pid)
    )
}

/// Formats a scroll event.
pub fn format_scroll(x: f64, y: f64, dx: f64, dy: f64, target_pid: Option<i32>) -> String {
    format!(
        r#"{{"kind":"scroll","x":{},"y":{},"dx":{},"dy":{}{}}}"#,
        fmt_num(x),
        fmt_num(y),
        fmt_num(dx),
        fmt_num(dy),
        target_pid_field(target_pid)
    )
}

/// Formats a keyboard text or combo event.
pub fn format_key(event: KeyEvent) -> String {
    match event {
        KeyEvent::Text(text) => format!(r#"{{"kind":"key","text":{}}}"#, quoted(&text)),
        KeyEvent::Combo(combo) => format!(r#"{{"kind":"key","combo":{}}}"#, quoted(&combo)),
    }
}

/// Formats a target highlight event.
pub fn format_target_set(bounds: &Rect, target_pid: Option<i32>) -> String {
    format!(
        r#"{{"kind":"target","x":{},"y":{},"w":{},"h":{}{}}}"#,
        fmt_num(bounds.x),
        fmt_num(bounds.y),
        fmt_num(bounds.width),
        fmt_num(bounds.height),
        target_pid_field(target_pid)
    )
}

/// Formats a target clear event.
pub fn format_target_clear() -> String {
    r#"{"kind":"target","clear":true}"#.to_string()
}

/// Formats an overlay error event.
pub fn format_error(point: Option<Point>, code: &str, message: &str) -> String {
    match point {
        Some(point) => format!(
            r#"{{"kind":"error","x":{},"y":{},"code":{},"message":{}}}"#,
            fmt_num(point.x),
            fmt_num(point.y),
            quoted(code),
            quoted(message)
        ),
        None => format!(
            r#"{{"kind":"error","code":{},"message":{}}}"#,
            quoted(code),
            quoted(message)
        ),
    }
}

/// Formats a thinking-state event.
pub fn format_thinking(active: bool) -> String {
    format!(r#"{{"kind":"thinking","thinking":{active}}}"#)
}

/// Formats an overlay visibility event.
pub fn format_set_visible(visible: bool) -> String {
    format!(r#"{{"kind":"set_visible","visible":{visible}}}"#)
}

/// Formats an overlay color event.
pub fn format_set_color(r: u8, g: u8, b: u8) -> String {
    format!(r#"{{"kind":"set_color","r":{r},"g":{g},"b":{b}}}"#)
}

/// Formats an overlay disconnect event.
pub fn format_bye() -> String {
    r#"{"kind":"bye"}"#.to_string()
}

/// Formats numeric wire values with integer compaction.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

/// Returns the wire label for a mouse button.
pub fn button_str(button: &MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
    }
}

fn target_pid_field(target_pid: Option<i32>) -> String {
    target_pid.map_or_else(String::new, |pid| format!(r#","target_pid":{pid}"#))
}

fn quoted(value: &str) -> String {
    serde_json::Value::String(value.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Rect {
        Rect {
            x: 1.0,
            y: 2.0,
            width: 30.5,
            height: 40.75,
        }
    }

    #[test]
    fn format_move() {
        assert_eq!(
            super::format_move(10.0, 20.5, Some(42)),
            r#"{"kind":"move","x":10,"y":20.50,"target_pid":42}"#
        );
        assert_eq!(
            super::format_move(10.0, 20.0, None),
            r#"{"kind":"move","x":10,"y":20}"#
        );
    }

    #[test]
    fn format_click() {
        assert_eq!(
            super::format_click(10.0, 20.0, &MouseButton::Right, 2, Some(7)),
            r#"{"kind":"click","x":10,"y":20,"button":"right","count":2,"target_pid":7}"#
        );
        assert_eq!(
            super::format_click(10.0, 20.0, &MouseButton::Left, 1, None),
            r#"{"kind":"click","x":10,"y":20,"button":"left","count":1}"#
        );
    }

    #[test]
    fn format_scroll() {
        assert_eq!(
            super::format_scroll(10.0, 20.0, 1.5, -3.0, Some(9)),
            r#"{"kind":"scroll","x":10,"y":20,"dx":1.50,"dy":-3,"target_pid":9}"#
        );
        assert_eq!(
            super::format_scroll(10.0, 20.0, 0.0, 4.25, None),
            r#"{"kind":"scroll","x":10,"y":20,"dx":0,"dy":4.25}"#
        );
    }

    #[test]
    fn format_key() {
        assert_eq!(
            super::format_key(KeyEvent::Text("hello".to_string())),
            r#"{"kind":"key","text":"hello"}"#
        );
    }

    #[test]
    fn format_target_set() {
        assert_eq!(
            super::format_target_set(&bounds(), Some(99)),
            r#"{"kind":"target","x":1,"y":2,"w":30.50,"h":40.75,"target_pid":99}"#
        );
        assert_eq!(
            super::format_target_set(&bounds(), None),
            r#"{"kind":"target","x":1,"y":2,"w":30.50,"h":40.75}"#
        );
    }

    #[test]
    fn format_target_clear() {
        assert_eq!(
            super::format_target_clear(),
            r#"{"kind":"target","clear":true}"#
        );
    }

    #[test]
    fn format_error() {
        assert_eq!(
            super::format_error(Some(Point { x: 10.0, y: 20.0 }), "STALE_REF", "refresh"),
            r#"{"kind":"error","x":10,"y":20,"code":"STALE_REF","message":"refresh"}"#
        );
    }

    #[test]
    fn format_thinking() {
        assert_eq!(
            super::format_thinking(true),
            r#"{"kind":"thinking","thinking":true}"#
        );
    }

    #[test]
    fn format_set_visible() {
        assert_eq!(
            super::format_set_visible(false),
            r#"{"kind":"set_visible","visible":false}"#
        );
    }

    #[test]
    fn format_set_color() {
        assert_eq!(
            super::format_set_color(1, 2, 3),
            r#"{"kind":"set_color","r":1,"g":2,"b":3}"#
        );
    }

    #[test]
    fn format_bye() {
        assert_eq!(super::format_bye(), r#"{"kind":"bye"}"#);
    }

    #[test]
    fn format_error_omits_point_when_none() {
        let without_point = super::format_error(None, "STALE_REF", "refresh");
        assert_eq!(
            without_point,
            r#"{"kind":"error","code":"STALE_REF","message":"refresh"}"#
        );
        assert!(!without_point.contains(r#""x""#));
        assert!(!without_point.contains(r#""y""#));

        let with_point =
            super::format_error(Some(Point { x: 10.0, y: 20.0 }), "STALE_REF", "refresh");
        assert!(with_point.contains(r#""x":10"#));
        assert!(with_point.contains(r#""y":20"#));
    }

    #[test]
    fn format_key_xor_text_combo() {
        let text = super::format_key(KeyEvent::Text("Cmd+S".to_string()));
        let combo = super::format_key(KeyEvent::Combo("Cmd+S".to_string()));
        assert_eq!(text, r#"{"kind":"key","text":"Cmd+S"}"#);
        assert_eq!(combo, r#"{"kind":"key","combo":"Cmd+S"}"#);
        assert_ne!(text, combo);
    }
}
