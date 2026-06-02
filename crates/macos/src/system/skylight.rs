#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use std::sync::OnceLock;

    const RTLD_LAZY: i32 = 1;
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;

    unsafe extern "C" {
        fn dlopen(path: *const i8, mode: i32) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
    }

    type GetProcessForPidFn = unsafe extern "C" fn(i32, *mut u8) -> i32;
    type SlpsPostEventRecordToFn = unsafe extern "C" fn(*const u8, *const u8) -> i32;

    struct Symbols {
        get_process_for_pid: GetProcessForPidFn,
        slps_post_event_record_to: SlpsPostEventRecordToFn,
    }

    fn symbols() -> Option<&'static Symbols> {
        static SYMS: OnceLock<Option<Symbols>> = OnceLock::new();
        SYMS.get_or_init(|| unsafe {
            let path = c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";
            let _ = dlopen(path.as_ptr(), RTLD_LAZY);
            let get_proc = dlsym(RTLD_DEFAULT, c"GetProcessForPID".as_ptr());
            let slps = dlsym(RTLD_DEFAULT, c"SLPSPostEventRecordTo".as_ptr());
            if get_proc.is_null() || slps.is_null() {
                return None;
            }
            Some(Symbols {
                get_process_for_pid: std::mem::transmute::<*mut c_void, GetProcessForPidFn>(
                    get_proc,
                ),
                slps_post_event_record_to: std::mem::transmute::<
                    *mut c_void,
                    SlpsPostEventRecordToFn,
                >(slps),
            })
        })
        .as_ref()
    }

    fn stamp_window_number(record: &mut [u8], offset: usize, window_number: u32) {
        record[offset..offset + 4].copy_from_slice(&window_number.to_le_bytes());
    }

    fn target_only_focus_record(window_number: u32) -> [u8; 0x100] {
        let mut record = [0u8; 0x100];
        record[0x04] = 0xF8;
        record[0x08] = 0x0D;
        stamp_window_number(&mut record, 0x3C, window_number);
        record[0x8A] = 0x01;
        record
    }

    fn key_window_records(window_number: u32) -> [[u8; 0x100]; 2] {
        let mut template = [0u8; 0x100];
        template[0x04] = 0xF8;
        template[0x3A] = 0x10;
        for byte in template.iter_mut().take(0x30).skip(0x20) {
            *byte = 0xFF;
        }
        stamp_window_number(&mut template, 0x3C, window_number);
        let mut phase1 = template;
        let mut phase2 = template;
        phase1[0x08] = 0x01;
        phase2[0x08] = 0x02;
        [phase1, phase2]
    }

    /// Prepare the target window for native CGEvent key delivery without
    /// raising the app. Returns `true` if all three SLPS records were
    /// accepted; `false` if SkyLight is unavailable on this macOS version
    /// or any record was rejected (caller should still proceed; the
    /// post-to-pid keyboard sequence may still land).
    pub fn preflight_window(pid: i32, window_number: u32) -> bool {
        let Some(syms) = symbols() else {
            return false;
        };
        let mut psn = [0u8; 8];
        unsafe {
            if (syms.get_process_for_pid)(pid, psn.as_mut_ptr()) != 0 {
                return false;
            }
            let focus = target_only_focus_record(window_number);
            if (syms.slps_post_event_record_to)(psn.as_ptr(), focus.as_ptr()) != 0 {
                return false;
            }
            for record in key_window_records(window_number) {
                if (syms.slps_post_event_record_to)(psn.as_ptr(), record.as_ptr()) != 0 {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn preflight_window(_pid: i32, _window_number: u32) -> bool {
        false
    }
}

pub use imp::preflight_window;
