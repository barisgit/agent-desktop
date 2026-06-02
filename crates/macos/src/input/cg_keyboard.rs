use agent_desktop_core::{
    action::{KeyCombo, Modifier},
    error::AdapterError,
};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_graphics::event::{CGEvent, CGEventFlags};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use foreign_types::ForeignType;

    pub fn post_combo_to_pid(combo: &KeyCombo, pid: i32) -> Result<(), AdapterError> {
        let key_code = key_to_keycode(&combo.key).ok_or_else(|| {
            AdapterError::new(
                agent_desktop_core::error::ErrorCode::ActionNotSupported,
                format!(
                    "No CGEvent keycode for key '{}'. Cannot synthesize via headless path.",
                    combo.key
                ),
            )
        })?;
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|()| AdapterError::internal("Failed to create CGEventSource"))?;
        unsafe {
            CGEventSourceSetLocalEventsSuppressionInterval(source.as_ptr() as *const _, 0.0);
        }

        let mut accumulated = CGEventFlags::CGEventFlagNull;
        for m in &combo.modifiers {
            accumulated |= modifier_flag(m);
            let ev = CGEvent::new_keyboard_event(source.clone(), modifier_keycode(m), true)
                .map_err(|()| AdapterError::internal("CGEvent modifier-down create failed"))?;
            ev.set_flags(accumulated);
            post_to_pid(&ev, pid);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let down = CGEvent::new_keyboard_event(source.clone(), key_code, true)
            .map_err(|()| AdapterError::internal("CGEvent key-down create failed"))?;
        let up = CGEvent::new_keyboard_event(source.clone(), key_code, false)
            .map_err(|()| AdapterError::internal("CGEvent key-up create failed"))?;
        down.set_flags(accumulated);
        up.set_flags(accumulated);
        post_to_pid(&down, pid);
        post_to_pid(&up, pid);
        std::thread::sleep(std::time::Duration::from_millis(10));

        for m in combo.modifiers.iter().rev() {
            accumulated &= !modifier_flag(m);
            let ev = CGEvent::new_keyboard_event(source.clone(), modifier_keycode(m), false)
                .map_err(|()| AdapterError::internal("CGEvent modifier-up create failed"))?;
            ev.set_flags(accumulated);
            post_to_pid(&ev, pid);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Ok(())
    }

    fn post_to_pid(event: &CGEvent, pid: i32) {
        unsafe {
            CGEventPostToPid(pid, event.as_ptr() as *const std::ffi::c_void);
        }
    }

    unsafe extern "C" {
        fn CGEventPostToPid(pid: i32, event: *const std::ffi::c_void);
        fn CGEventSourceSetLocalEventsSuppressionInterval(
            source: *const std::ffi::c_void,
            seconds: f64,
        );
    }

    fn modifier_flag(m: &Modifier) -> CGEventFlags {
        match m {
            Modifier::Cmd => CGEventFlags::CGEventFlagCommand,
            Modifier::Shift => CGEventFlags::CGEventFlagShift,
            Modifier::Alt => CGEventFlags::CGEventFlagAlternate,
            Modifier::Ctrl => CGEventFlags::CGEventFlagControl,
        }
    }

    fn modifier_keycode(m: &Modifier) -> u16 {
        match m {
            Modifier::Cmd => 55,
            Modifier::Shift => 56,
            Modifier::Alt => 58,
            Modifier::Ctrl => 59,
        }
    }

    fn key_to_keycode(key: &str) -> Option<u16> {
        Some(match key {
            "a" => 0,
            "b" => 11,
            "c" => 8,
            "d" => 2,
            "e" => 14,
            "f" => 3,
            "g" => 5,
            "h" => 4,
            "i" => 34,
            "j" => 38,
            "k" => 40,
            "l" => 37,
            "m" => 46,
            "n" => 45,
            "o" => 31,
            "p" => 35,
            "q" => 12,
            "r" => 15,
            "s" => 1,
            "t" => 17,
            "u" => 32,
            "v" => 9,
            "w" => 13,
            "x" => 7,
            "y" => 16,
            "z" => 6,
            "0" => 29,
            "1" => 18,
            "2" => 19,
            "3" => 20,
            "4" => 21,
            "5" => 23,
            "6" => 22,
            "7" => 26,
            "8" => 28,
            "9" => 25,
            "return" | "enter" => 36,
            "escape" | "esc" => 53,
            "tab" => 48,
            "space" => 49,
            "delete" | "backspace" => 51,
            "forwarddelete" => 117,
            "home" => 115,
            "end" => 119,
            "pageup" => 116,
            "pagedown" => 121,
            "left" => 123,
            "right" => 124,
            "down" => 125,
            "up" => 126,
            "f1" => 122,
            "f2" => 120,
            "f3" => 99,
            "f4" => 118,
            "f5" => 96,
            "f6" => 97,
            "f7" => 98,
            "f8" => 100,
            "f9" => 101,
            "f10" => 109,
            "f11" => 103,
            "f12" => 111,
            _ => return None,
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn post_combo_to_pid(_combo: &KeyCombo, _pid: i32) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("post_combo_to_pid"))
    }
}

pub use imp::post_combo_to_pid;
