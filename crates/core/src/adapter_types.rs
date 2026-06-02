use crate::{
    element_state::ElementState,
    error::{AdapterError, ErrorCode},
    node::Rect,
};
use std::marker::PhantomData;

pub struct HitTestResult {
    pub handle: NativeHandle,
    pub role: String,
    pub name: Option<String>,
    pub bounds: Option<Rect>,
    pub bounds_hash: Option<u64>,
    pub available_actions: Vec<String>,
    pub pid: Option<i32>,
}

pub struct WindowFilter {
    pub focused_only: bool,
    pub app: Option<String>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSurface {
    #[default]
    Window,
    Focused,
    Menu,
    Menubar,
    Sheet,
    Popover,
    Alert,
}

impl SnapshotSurface {
    pub fn is_window(surface: &Self) -> bool {
        matches!(surface, Self::Window)
    }
}

#[derive(Clone, Copy)]
pub struct TreeOptions {
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: SnapshotSurface,
    pub skeleton: bool,
}

impl Default for TreeOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            include_bounds: false,
            interactive_only: false,
            compact: false,
            surface: SnapshotSurface::Window,
            skeleton: false,
        }
    }
}

impl TreeOptions {
    pub(crate) fn with_ref_identity_bounds(mut self) -> Self {
        self.include_bounds = true;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct LiveElement {
    pub state: Option<ElementState>,
    pub bounds: Option<Rect>,
    pub available_actions: Option<Vec<String>>,
}

pub(crate) fn optional_live_read<T>(
    result: Result<Option<T>, AdapterError>,
) -> Result<Option<T>, AdapterError> {
    match result {
        Ok(value) => Ok(value),
        Err(err) if is_live_read_unsupported(&err) => Ok(None),
        Err(err) => Err(err),
    }
}

fn is_live_read_unsupported(err: &AdapterError) -> bool {
    matches!(
        err.code,
        ErrorCode::PlatformNotSupported | ErrorCode::ActionNotSupported
    )
}

pub enum ScreenshotTarget {
    Screen(usize),
    /// Capture the largest visible window owned by this process ID.
    Window(i32),
    FullScreen,
}

pub struct NativeHandle {
    pub(crate) ptr: *const std::ffi::c_void,
    _not_send_sync: PhantomData<*const ()>,
}

impl NativeHandle {
    /// # Safety
    ///
    /// `ptr` must be a valid platform accessibility handle whose ownership is
    /// transferred to the caller. The adapter that creates the handle must
    /// document how it is released through [`PlatformAdapter::release_handle`].
    pub unsafe fn from_ptr(ptr: *const std::ffi::c_void) -> Self {
        Self {
            ptr,
            _not_send_sync: PhantomData,
        }
    }

    pub fn null() -> Self {
        Self {
            ptr: std::ptr::null(),
            _not_send_sync: PhantomData,
        }
    }
}

impl NativeHandle {
    /// Returns the raw platform pointer. For use by platform adapter crates only.
    /// Callers must not retain the pointer beyond the lifetime of this handle.
    pub fn as_raw(&self) -> *const std::ffi::c_void {
        self.ptr
    }
}

pub struct ImageBuffer {
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

pub enum ImageFormat {
    Png,
    Jpg,
}

impl ImageFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpg => "jpg",
        }
    }
}
