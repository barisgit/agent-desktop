use agent_desktop_core::{action::KeyCombo, action_result::ActionResult, error::AdapterError};

#[cfg(target_os = "macos")]
use agent_desktop_core::{action::Modifier, adapter::WindowFilter};

#[cfg(target_os = "macos")]
pub fn press_for_app_impl(app_name: &str, combo: &KeyCombo) -> Result<ActionResult, AdapterError> {
    tracing::debug!("system: press_for_app app={app_name:?} key={:?}", combo.key);
    let pid = find_pid_by_name(app_name)?;
    press_for_pid_impl(pid, combo, true)
}

#[cfg(target_os = "macos")]
pub fn press_for_pid_impl(
    pid: i32,
    combo: &KeyCombo,
    steal_focus: bool,
) -> Result<ActionResult, AdapterError> {
    press_for_pid_with_window_impl(pid, None, combo, steal_focus)
}

#[cfg(target_os = "macos")]
pub fn press_for_pid_with_window_impl(
    pid: i32,
    window_number: Option<u32>,
    combo: &KeyCombo,
    steal_focus: bool,
) -> Result<ActionResult, AdapterError> {
    let app_el = crate::tree::element_for_pid(pid);
    if app_el.0.is_null() {
        return Err(AdapterError::internal("Failed to create AX app element"));
    }

    if steal_focus {
        if let Err(err) = crate::system::app_ops::ensure_app_focused(pid) {
            tracing::debug!("press_for_pid: focus before key dispatch failed: {err}");
        }

        if !combo.modifiers.is_empty()
            && let Some(result) = try_menu_bar_shortcut(&app_el, combo)
        {
            return result;
        }

        if let Some(result) = try_simple_key_action(app_el.0, combo) {
            return result;
        }

        ax_post_keyboard_event(app_el.0, combo)?;
        return Ok(ActionResult::new("press_key"));
    }

    if let Some(window_number) =
        window_number.or_else(|| crate::system::cg_window::find_cg_window_id_for_pid(pid))
    {
        let preflight = crate::system::skylight::preflight_window(pid, window_number);
        tracing::debug!(
            "press_for_pid headless preflight pid={pid} window={window_number} ok={preflight}"
        );
    }
    crate::input::cg_keyboard::post_combo_to_pid(combo, pid)?;
    Ok(ActionResult::new("press_key"))
}

#[cfg(target_os = "macos")]
fn try_simple_key_action(
    app_el: accessibility_sys::AXUIElementRef,
    combo: &KeyCombo,
) -> Option<Result<ActionResult, AdapterError>> {
    use accessibility_sys::{AXUIElementPerformAction, kAXErrorSuccess};
    use core_foundation::{base::TCFType, string::CFString};

    if !combo.modifiers.is_empty() {
        return None;
    }

    let focused = get_focused_element(app_el)?;
    let action_name = match combo.key.as_str() {
        "return" | "enter" => "AXConfirm",
        "escape" | "esc" => "AXCancel",
        "space" => "AXPress",
        _ => return None,
    };

    let ax_action = CFString::new(action_name);
    let err = unsafe { AXUIElementPerformAction(focused.0, ax_action.as_concrete_TypeRef()) };
    if err == kAXErrorSuccess {
        Some(Ok(ActionResult::new("press_key".to_string())))
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn get_focused_element(
    app_el: accessibility_sys::AXUIElementRef,
) -> Option<crate::tree::AXElement> {
    use accessibility_sys::{AXUIElementCopyAttributeValue, kAXErrorSuccess};
    use core_foundation::{base::TCFType, string::CFString};

    let attr = CFString::new("AXFocusedUIElement");
    let mut value: core_foundation_sys::base::CFTypeRef = std::ptr::null_mut();
    let err =
        unsafe { AXUIElementCopyAttributeValue(app_el, attr.as_concrete_TypeRef(), &mut value) };
    if err != kAXErrorSuccess || value.is_null() {
        return None;
    }
    crate::tree::ax_value::created_ax_element(value)
}

#[cfg(target_os = "macos")]
fn try_menu_bar_shortcut(
    app_el: &crate::tree::AXElement,
    combo: &KeyCombo,
) -> Option<Result<ActionResult, AdapterError>> {
    use accessibility_sys::{AXUIElementPerformAction, kAXErrorSuccess};
    use core_foundation::{base::TCFType, string::CFString};

    let menu_bar = crate::tree::copy_element_attr(app_el, "AXMenuBar")?;
    let menu_bar_items = crate::tree::copy_ax_array(&menu_bar, "AXChildren")?;

    let target_char = if combo.key.len() == 1 {
        combo.key.to_uppercase()
    } else {
        return None;
    };

    let target_mods = combo_to_ax_modifiers(combo);

    for bar_item in &menu_bar_items {
        if let Some(menu) = crate::tree::copy_ax_array(bar_item, "AXChildren") {
            for menu_group in &menu {
                if let Some(items) = crate::tree::copy_ax_array(menu_group, "AXChildren") {
                    for item in &items {
                        let cmd_char = crate::tree::copy_string_attr(item, "AXMenuItemCmdChar");
                        let cmd_mods = read_menu_item_modifiers(item);

                        if let Some(ch) = &cmd_char {
                            if ch.to_uppercase() == target_char && cmd_mods == target_mods {
                                let press = CFString::new("AXPress");
                                let err = unsafe {
                                    AXUIElementPerformAction(item.0, press.as_concrete_TypeRef())
                                };
                                if err == kAXErrorSuccess {
                                    return Some(Ok(ActionResult::new("press_key".to_string())));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn read_menu_item_modifiers(el: &crate::tree::AXElement) -> u32 {
    use accessibility_sys::{AXUIElementCopyAttributeValue, kAXErrorSuccess};
    use core_foundation::{base::TCFType, string::CFString};

    let attr = CFString::new("AXMenuItemCmdModifiers");
    let mut value: core_foundation_sys::base::CFTypeRef = std::ptr::null_mut();
    let err =
        unsafe { AXUIElementCopyAttributeValue(el.0, attr.as_concrete_TypeRef(), &mut value) };
    if err != kAXErrorSuccess || value.is_null() {
        return 0;
    }
    let cf = unsafe { core_foundation::base::CFType::wrap_under_create_rule(value) };
    crate::cf_type::borrowed_cf_number(cf.as_concrete_TypeRef())
        .and_then(|number| number.to_i64())
        .map(|v| v as u32)
        .unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn combo_to_ax_modifiers(combo: &KeyCombo) -> u32 {
    let mut mods: u32 = 0;
    for m in &combo.modifiers {
        match m {
            Modifier::Shift => mods |= 1 << 0,
            Modifier::Alt => mods |= 1 << 1,
            Modifier::Ctrl => mods |= 1 << 2,
            Modifier::Cmd => {}
        }
    }
    mods
}

#[cfg(target_os = "macos")]
fn ax_post_keyboard_event(
    app_el: accessibility_sys::AXUIElementRef,
    combo: &KeyCombo,
) -> Result<(), AdapterError> {
    use accessibility_sys::AXUIElementPostKeyboardEvent;

    let key_code = key_to_keycode(&combo.key).ok_or_else(|| {
        AdapterError::new(
            agent_desktop_core::error::ErrorCode::ActionNotSupported,
            format!(
                "No AX equivalent for key combo '{}'. This combo has no menu-bar action.",
                format_combo(combo)
            ),
        )
        .with_suggestion("This key combo cannot be executed via accessibility APIs alone.")
    })?;

    let err = unsafe { AXUIElementPostKeyboardEvent(app_el, 0 as _, key_code, true) };
    if err != accessibility_sys::kAXErrorSuccess {
        return Err(AdapterError::internal(format!(
            "AXUIElementPostKeyboardEvent key-down failed (err={err})"
        )));
    }

    let err = unsafe { AXUIElementPostKeyboardEvent(app_el, 0 as _, key_code, false) };
    if err != accessibility_sys::kAXErrorSuccess {
        return Err(AdapterError::internal(format!(
            "AXUIElementPostKeyboardEvent key-up failed (err={err})"
        )));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn format_combo(combo: &KeyCombo) -> String {
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

#[cfg(target_os = "macos")]
fn key_to_keycode(key: &str) -> Option<u16> {
    match key {
        "cmd" | "command" | "shift" | "alt" | "option" | "ctrl" | "control" => None,
        other => crate::input::keyboard_map::key_name_to_code(other).ok(),
    }
}

#[cfg(target_os = "macos")]
pub fn press_at_element_impl(
    element: &crate::tree::AXElement,
    combo: &KeyCombo,
) -> Result<ActionResult, AdapterError> {
    use accessibility_sys::{AXUIElementPerformAction, kAXErrorSuccess};
    use core_foundation::{base::TCFType, string::CFString};

    if combo.modifiers.is_empty() {
        let action_name = match combo.key.as_str() {
            "return" | "enter" => Some("AXConfirm"),
            "escape" | "esc" => Some("AXCancel"),
            "space" => Some("AXPress"),
            _ => None,
        };
        if let Some(action_name) = action_name {
            let action = CFString::new(action_name);
            let result =
                unsafe { AXUIElementPerformAction(element.0, action.as_concrete_TypeRef()) };
            if result == kAXErrorSuccess {
                return Ok(ActionResult::new("press_key"));
            }
        }
    }

    let pid = crate::system::app_ops::pid_from_element(element)
        .ok_or_else(|| AdapterError::internal("Could not determine pid for key delivery"))?;
    let app = crate::tree::element_for_pid(pid);
    if app.0.is_null() {
        return Err(AdapterError::internal(
            "Failed to create AX app element for key delivery",
        ));
    }
    ax_post_keyboard_event(app.0, combo)?;
    Ok(ActionResult::new("press_key"))
}

#[cfg(target_os = "macos")]
pub(crate) fn find_pid_by_name(app_name: &str) -> Result<i32, AdapterError> {
    let filter = WindowFilter {
        focused_only: false,
        app: Some(app_name.to_string()),
    };
    let windows = crate::system::window_list::list_windows_impl(&filter)?;
    windows
        .first()
        .map(|w| w.pid)
        .or_else(|| crate::system::app_list::pid_for_app_name(app_name))
        .ok_or_else(|| {
            AdapterError::new(
                agent_desktop_core::error::ErrorCode::AppNotFound,
                format!("App '{app_name}' not found"),
            )
            .with_suggestion(
                "Verify the app is running. Use 'list-apps' to see running applications.",
            )
        })
}

#[cfg(not(target_os = "macos"))]
pub fn press_for_app_impl(
    _app_name: &str,
    _combo: &KeyCombo,
) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("press_for_app"))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn find_pid_by_name(_app_name: &str) -> Result<i32, AdapterError> {
    Err(AdapterError::not_supported("find_pid_by_name"))
}
