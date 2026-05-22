use agent_desktop_core::action::{KeyCombo, Modifier};

pub(crate) fn format_combo(combo: &KeyCombo) -> String {
    let mods: Vec<&str> = combo
        .modifiers
        .iter()
        .map(|m| match m {
            Modifier::Cmd => "cmd",
            Modifier::Ctrl => "ctrl",
            Modifier::Alt => "alt",
            Modifier::Shift => "shift",
        })
        .collect();
    if mods.is_empty() {
        combo.key.clone()
    } else {
        format!("{}+{}", mods.join("+"), combo.key)
    }
}

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
