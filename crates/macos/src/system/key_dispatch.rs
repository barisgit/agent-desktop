use agent_desktop_core::{action::KeyCombo, action_result::ActionResult, error::AdapterError};

#[cfg(target_os = "macos")]
use crate::system::key_dispatch_ax::{
    ax_post_keyboard_event, try_menu_bar_shortcut, try_simple_key_action,
};
#[cfg(target_os = "macos")]
use agent_desktop_core::adapter::WindowFilter;

#[cfg(target_os = "macos")]
pub fn press_for_app_impl(app_name: &str, combo: &KeyCombo) -> Result<ActionResult, AdapterError> {
    tracing::debug!("system: press_for_app app={app_name:?} key={:?}", combo.key);
    let pid = find_pid_by_name(app_name)?;
    press_for_pid_impl(pid, combo, true)
}

/// Press `combo` against the app identified by `pid`.
///
/// When `steal_focus` is true, AXFrontmost is set on the app first (the
/// classic disruptive path). When false (headless), focus is left alone
/// and the press is delivered via `AXConfirm`/`AXCancel`/`AXPress` on
/// the currently focused element (within the target app) or via
/// `AXUIElementPostKeyboardEvent` against the app element. Menu-bar
/// shortcuts are skipped under headless because invoking a menu item
/// can in some apps cause a frontmost transition.
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
    tracing::debug!(
        "system: press_for_pid pid={pid} key={:?} steal_focus={steal_focus}",
        combo.key
    );
    let app_el = crate::tree::element_for_pid(pid);
    if app_el.0.is_null() {
        return Err(AdapterError::internal("Failed to create AX app element"));
    }

    if steal_focus {
        if let Err(err) = crate::system::app_ops::ensure_app_focused(pid) {
            tracing::debug!("press_for_pid: focus before key dispatch failed: {err}");
        }

        if !combo.modifiers.is_empty() {
            if let Some(result) = try_menu_bar_shortcut(&app_el, combo) {
                return result;
            }
        }

        let simple_result = try_simple_key_action(app_el.0, combo);
        if let Some(result) = simple_result {
            return result;
        }

        ax_post_keyboard_event(app_el.0, combo)?;
        return Ok(ActionResult::new("press_key".to_string()));
    }

    let resolved_window =
        window_number.or_else(|| crate::system::cg_window::find_cg_window_id_for_pid(pid));

    if let Some(window_number) = resolved_window {
        if let Some(key_window) = focused_cg_window(pid) {
            if key_window != window_number {
                return Err(AdapterError::new(
                    agent_desktop_core::error::ErrorCode::ActionFailed,
                    format!(
                        "Key delivery target window w-{window_number} is not the app's key \
                         window (currently w-{key_window}); the keystroke would land in the \
                         wrong window"
                    ),
                )
                .with_suggestion(
                    "This app has multiple windows on one process and the target is not key. \
                     Send the key to the key window, use a ref-based action (click/set-value/\
                     press <REF>) that targets the element directly, or focus the target window \
                     first if a brief activation is acceptable.",
                ));
            }
        }

        let _preflight = crate::system::skylight::preflight_window(pid, window_number);
        tracing::debug!(
            "press_for_pid headless preflight pid={pid} window={window_number} ok={_preflight}"
        );
    }
    crate::input::cg_keyboard::post_combo_to_pid(combo, pid)?;
    Ok(ActionResult::new("press_key".to_string()))
}

/// Returns the CGWindowID of the app's currently focused AXWindow, or
/// `None` if it cannot be determined. Used to verify that headless key
/// delivery will land in the intended window before sending input.
#[cfg(target_os = "macos")]
fn focused_cg_window(pid: i32) -> Option<u32> {
    let app = crate::tree::element_for_pid(pid);
    if app.0.is_null() {
        return None;
    }
    let focused = crate::tree::copy_element_attr(&app, "AXFocusedWindow")?;
    crate::tree::builder::ax_cg_window_id(&focused)
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
pub fn press_for_pid_impl(
    _pid: i32,
    _combo: &KeyCombo,
    _steal_focus: bool,
) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("press_for_pid"))
}

/// Press `combo` directly at `element`, with zero side effects on focus.
///
/// Modifierless `return`/`enter` -> AXConfirm, `escape`/`esc` -> AXCancel,
/// `space` -> AXPress against the element. For everything else (and on
/// AX action failure) fall through to `AXUIElementPostKeyboardEvent` on
/// the element's owning app. Never sets AXFrontmost.
#[cfg(target_os = "macos")]
pub fn press_at_element_impl(
    element: &crate::tree::AXElement,
    combo: &KeyCombo,
) -> Result<ActionResult, AdapterError> {
    use accessibility_sys::{AXUIElementPerformAction, kAXErrorSuccess};
    use core_foundation::{base::TCFType, string::CFString};

    tracing::debug!("system: press_at_element key={:?}", combo.key);

    if combo.modifiers.is_empty() {
        let action_name = match combo.key.as_str() {
            "return" | "enter" => Some("AXConfirm"),
            "escape" | "esc" => Some("AXCancel"),
            "space" => Some("AXPress"),
            _ => None,
        };
        if let Some(name) = action_name {
            let ax_action = CFString::new(name);
            let err =
                unsafe { AXUIElementPerformAction(element.0, ax_action.as_concrete_TypeRef()) };
            if err == kAXErrorSuccess {
                return Ok(ActionResult::new("press_key".to_string()));
            }
        }
    }

    let pid = crate::system::app_ops::pid_from_element(element).ok_or_else(|| {
        AdapterError::internal("Could not determine pid for press_at_element fallback")
    })?;
    let app_el = crate::tree::element_for_pid(pid);
    if app_el.0.is_null() {
        return Err(AdapterError::internal(
            "Failed to create AX app element for press_at_element fallback",
        ));
    }
    ax_post_keyboard_event(app_el.0, combo)?;
    Ok(ActionResult::new("press_key".to_string()))
}

#[cfg(not(target_os = "macos"))]
pub fn press_at_element_impl(
    _element: &(),
    _combo: &KeyCombo,
) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("press_at_element"))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn find_pid_by_name(_app_name: &str) -> Result<i32, AdapterError> {
    Err(AdapterError::not_supported("find_pid_by_name"))
}
