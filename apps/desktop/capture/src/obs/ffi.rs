//! Small safe-ish helpers over the libobs C API.

use std::ffi::{c_char, c_void, CStr, CString};

use libobs as sys;

pub fn cstr(text: &str) -> CString {
    CString::new(text.replace('\0', "")).expect("no interior nul")
}

/// An `obs_data_t` built from JSON, released on drop.
pub struct Data(pub *mut sys::obs_data_t);

impl Data {
    pub fn from_json(value: &serde_json::Value) -> Self {
        let json = cstr(&value.to_string());
        Data(unsafe { sys::obs_data_create_from_json(json.as_ptr()) })
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { sys::obs_data_release(self.0) };
        }
    }
}

pub fn source_create(id: &str, name: &str, settings: &serde_json::Value) -> Option<*mut sys::obs_source_t> {
    let (id_c, name_c) = (cstr(id), cstr(name));
    let data = Data::from_json(settings);
    let source = unsafe { sys::obs_source_create(id_c.as_ptr(), name_c.as_ptr(), data.0, std::ptr::null_mut()) };
    (!source.is_null()).then_some(source)
}

pub fn source_update(source: *mut sys::obs_source_t, settings: &serde_json::Value) {
    let data = Data::from_json(settings);
    unsafe { sys::obs_source_update(source, data.0) };
}

fn owned(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
}

pub fn string(text: *const c_char) -> String {
    owned(text)
}

/// One entry of a list property: what it shows and the value to store.
#[derive(Debug, Clone)]
pub struct ListItem {
    pub name: String,
    pub value: String,
}

/// Items of a list property (devices, monitors, windows), or empty when it has none.
pub fn list_items(source: *mut sys::obs_source_t, property: &str) -> Vec<ListItem> {
    let name = cstr(property);
    let mut items = Vec::new();
    unsafe {
        let props = sys::obs_source_properties(source);
        if props.is_null() {
            return items;
        }
        let prop = sys::obs_properties_get(props, name.as_ptr());
        if !prop.is_null() {
            let format = sys::obs_property_list_format(prop);
            for index in 0..sys::obs_property_list_item_count(prop) {
                let value = match format {
                    sys::obs_combo_format_OBS_COMBO_FORMAT_STRING => owned(sys::obs_property_list_item_string(prop, index)),
                    sys::obs_combo_format_OBS_COMBO_FORMAT_INT => sys::obs_property_list_item_int(prop, index).to_string(),
                    _ => continue,
                };
                items.push(ListItem { name: owned(sys::obs_property_list_item_name(prop, index)), value });
            }
        }
        sys::obs_properties_destroy(props);
    }
    items
}

pub fn vec2(x: f32, y: f32) -> sys::vec2 {
    sys::vec2 { __bindgen_anon_1: sys::vec2__bindgen_ty_1 { ptr: [x, y] } }
}

/// Calls a procedure on a handler with empty parameters; returns the filled calldata.
pub struct Calldata(sys::calldata_t);

impl Calldata {
    pub fn new() -> Self {
        // calldata_init is an inline memset.
        Calldata(unsafe { std::mem::zeroed() })
    }

    pub fn as_mut(&mut self) -> *mut sys::calldata_t {
        &mut self.0
    }

    pub fn string(&self, name: &str) -> Option<String> {
        let key = cstr(name);
        let mut out: *const c_char = std::ptr::null();
        unsafe { sys::calldata_get_string(&self.0, key.as_ptr(), &mut out) }.then(|| owned(out))
    }
}

impl Drop for Calldata {
    fn drop(&mut self) {
        // calldata_free is inline too.
        if !self.0.fixed && !self.0.stack.is_null() {
            unsafe { sys::bfree(self.0.stack as *mut c_void) };
        }
    }
}

/// Reads an integer parameter from a signal's calldata (`calldata_int` is inline in C).
///
/// # Safety
/// `data` must be the calldata OBS passed to a signal callback.
pub unsafe fn calldata_int(data: *const sys::calldata_t, name: &str) -> i64 {
    let key = cstr(name);
    let mut value: i64 = 0;
    sys::calldata_get_data(data, key.as_ptr(), &mut value as *mut i64 as *mut c_void, std::mem::size_of::<i64>());
    value
}

/// Reads a pointer parameter, like `calldata_ptr` in C.
///
/// # Safety
/// `data` must be the calldata OBS passed to a signal callback, and `name` must hold a
/// pointer to `T`.
pub unsafe fn calldata_ptr<T>(data: *const sys::calldata_t, name: &str) -> *mut T {
    let key = cstr(name);
    let mut value: *mut T = std::ptr::null_mut();
    sys::calldata_get_data(data, key.as_ptr(), &mut value as *mut *mut T as *mut c_void, std::mem::size_of::<*mut T>());
    value
}

pub fn proc_call(handler: *mut sys::proc_handler_t, name: &str) -> Option<Calldata> {
    let key = cstr(name);
    let mut data = Calldata::new();
    unsafe { sys::proc_handler_call(handler, key.as_ptr(), data.as_mut()) }.then_some(data)
}

extern "C" {
    /// What MSVC's inline `vsnprintf` calls in the UCRT.
    fn __stdio_common_vsprintf(
        options: u64,
        buffer: *mut c_char,
        count: usize,
        format: *const c_char,
        locale: *mut c_void,
        args: sys::va_list,
    ) -> i32;
}

/// `_CRT_INTERNAL_PRINTF_STANDARD_SNPRINTF_BEHAVIOR`: truncate and always terminate.
const STANDARD_SNPRINTF: u64 = 2;

/// OBS's own log, forwarded to the app.
unsafe extern "C" fn log_handler(level: i32, message: *const c_char, args: sys::va_list, _param: *mut c_void) {
    let mut buffer = [0 as c_char; 4096];
    let written = __stdio_common_vsprintf(
        STANDARD_SNPRINTF,
        buffer.as_mut_ptr(),
        buffer.len(),
        message,
        std::ptr::null_mut(),
        args,
    );
    let text = if written < 0 { owned(message) } else { owned(buffer.as_ptr()) };
    match level {
        l if l <= sys::LOG_ERROR => log::error!("[obs] {text}"),
        l if l <= sys::LOG_WARNING => log::warn!("[obs] {text}"),
        l if l <= sys::LOG_INFO => log::info!("[obs] {text}"),
        _ => {}
    }
}

pub fn install_log_handler() {
    unsafe { sys::base_set_log_handler(Some(log_handler), std::ptr::null_mut()) };
}
