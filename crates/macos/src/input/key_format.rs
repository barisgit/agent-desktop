pub(crate) use agent_desktop_core::action::format_combo;

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::action::{KeyCombo, Modifier};

    #[test]
    fn formats_combo_with_modifiers() {
        let combo = KeyCombo {
            modifiers: vec![Modifier::Cmd, Modifier::Shift],
            key: "s".to_string(),
        };

        assert_eq!(format_combo(&combo), "cmd+shift+s");
    }

    #[test]
    fn formats_plain_key() {
        let combo = KeyCombo {
            modifiers: Vec::new(),
            key: "escape".to_string(),
        };

        assert_eq!(format_combo(&combo), "escape");
    }
}
