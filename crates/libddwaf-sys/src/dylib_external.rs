#![allow(clippy::missing_safety_doc)]

use std::sync::OnceLock;

use crate::*;

static LIBRARY: OnceLock<ddwaf> = OnceLock::new();

/// Load the libddwaf shared library from the given path.
///
/// Must be called before any libddwaf functions are used. Subsequent calls are
/// no-ops: the first successful load wins.
///
/// # Errors
/// Returns an error if the library at `path` cannot be opened or if any
/// required symbol is missing.
pub fn load(path: impl AsRef<std::path::Path>) -> Result<(), libloading::Error> {
    if LIBRARY.get().is_some() {
        return Ok(());
    }
    tracing::debug!(
        "loading libddwaf shared object from {}",
        path.as_ref().display()
    );
    let lib = unsafe { ddwaf::new(path.as_ref()) }?;
    let _ = LIBRARY.set(lib);
    Ok(())
}

/// Re-exports a function from the global [`ddwaf`] instance, returning the
/// fallback value when [`load`] has not been called yet or has not succeeded.
macro_rules! reexport {
    (
        $($vis:vis unsafe fn $name:ident($($arg_name:ident: $arg_type: ty),*) $(-> $ret_type:ty)? { $($fallback:expr)? })*
    ) => {
        $(
            $vis unsafe extern "C" fn $name($($arg_name: $arg_type),*) $(-> $ret_type)? {
                match LIBRARY.get() {
                    Some(lib) => unsafe { lib.$name($($arg_name),*) },
                    None => { $($fallback)? },
                }
            }
        )*
    };
}

// Please keep this list alphanumerically sorted for convenience.
reexport! {
    pub unsafe fn ddwaf_allocator_alloc(alloc: ddwaf_allocator, bytes: usize, alignment: usize) -> *mut ::std::os::raw::c_void { std::ptr::null_mut() }
    pub unsafe fn ddwaf_allocator_destroy(alloc: ddwaf_allocator) {}
    pub unsafe fn ddwaf_allocator_free(alloc: ddwaf_allocator, p: *mut ::std::os::raw::c_void, bytes: usize, alignment: usize) {}
    pub unsafe fn ddwaf_builder_add_or_update_config(builder: ddwaf_builder, path: *const std::os::raw::c_char, path_len: u32, config: *const ddwaf_object, diagnostics: *mut ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_builder_build_instance(builder: ddwaf_builder) -> ddwaf_handle { std::ptr::null_mut() }
    pub unsafe fn ddwaf_builder_destroy(builder: ddwaf_builder) {}
    pub unsafe fn ddwaf_builder_get_config_paths(builder: ddwaf_builder, paths: *mut ddwaf_object, filter: *const ::std::os::raw::c_char, filter_len: u32) -> u32 { 0 }
    pub unsafe fn ddwaf_builder_init() -> ddwaf_builder { std::ptr::null_mut() }
    pub unsafe fn ddwaf_builder_remove_config(builder: ddwaf_builder, path: *const std::os::raw::c_char, path_len: u32) -> bool { false }
    pub unsafe fn ddwaf_context_destroy(context: ddwaf_context) {}
    pub unsafe fn ddwaf_context_eval(context: ddwaf_context, data: *mut ddwaf_object, alloc: ddwaf_allocator, result: *mut ddwaf_object, timeout: u64) -> DDWAF_RET_CODE { DDWAF_ERR_INTERNAL }
    pub unsafe fn ddwaf_context_init(handle: ddwaf_handle, output_alloc: ddwaf_allocator) -> ddwaf_context { std::ptr::null_mut() }
    pub unsafe fn ddwaf_destroy(handle: ddwaf_handle) {}
    pub unsafe fn ddwaf_get_default_allocator() -> ddwaf_allocator { std::ptr::null_mut() }
    pub unsafe fn ddwaf_get_version() -> *const std::os::raw::c_char { std::ptr::null() }
    pub unsafe fn ddwaf_init(ruleset: *const ddwaf_object, diagnostics: *mut ddwaf_object) -> ddwaf_handle { std::ptr::null_mut() }
    pub unsafe fn ddwaf_known_actions(handle: ddwaf_handle, size: *mut u32) -> *const *const ::std::os::raw::c_char { std::ptr::null() }
    pub unsafe fn ddwaf_known_addresses(handle: ddwaf_handle, size: *mut u32) -> *const *const ::std::os::raw::c_char { std::ptr::null() }
    pub unsafe fn ddwaf_monotonic_allocator_init() -> ddwaf_allocator { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_at_key(object: *const ddwaf_object, index: usize) -> *const ddwaf_object { std::ptr::null() }
    pub unsafe fn ddwaf_object_at_value(object: *const ddwaf_object, index: usize) -> *const ddwaf_object { std::ptr::null() }
    pub unsafe fn ddwaf_object_clone(source: *const ddwaf_object, destination: *mut ddwaf_object, alloc: ddwaf_allocator) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_destroy(object: *mut ddwaf_object, alloc: ddwaf_allocator) {}
    pub unsafe fn ddwaf_object_find(object: *const ddwaf_object, key: *const ::std::os::raw::c_char, length: usize) -> *const ddwaf_object { std::ptr::null() }
    pub unsafe fn ddwaf_object_from_json(output: *mut ddwaf_object, json_str: *const std::os::raw::c_char, length: u32, alloc: ddwaf_allocator) -> bool { false }
    pub unsafe fn ddwaf_object_get_bool(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_get_float(object: *const ddwaf_object) -> f64 { 0.0 }
    pub unsafe fn ddwaf_object_get_length(object: *const ddwaf_object) -> usize { 0 }
    pub unsafe fn ddwaf_object_get_signed(object: *const ddwaf_object) -> i64 { 0 }
    pub unsafe fn ddwaf_object_get_size(object: *const ddwaf_object) -> usize { 0 }
    pub unsafe fn ddwaf_object_get_string(object: *const ddwaf_object, length: *mut usize) -> *const ::std::os::raw::c_char { std::ptr::null() }
    pub unsafe fn ddwaf_object_get_type(object: *const ddwaf_object) -> DDWAF_OBJ_TYPE { DDWAF_OBJ_INVALID }
    pub unsafe fn ddwaf_object_get_unsigned(object: *const ddwaf_object) -> u64 { 0 }
    pub unsafe fn ddwaf_object_insert(array: *mut ddwaf_object, alloc: ddwaf_allocator) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_insert_key(map: *mut ddwaf_object, key: *const ::std::os::raw::c_char, length: u32, alloc: ddwaf_allocator) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_insert_key_nocopy(map: *mut ddwaf_object, key: *const ::std::os::raw::c_char, length: u32, alloc: ddwaf_allocator) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_insert_literal_key(map: *mut ddwaf_object, key: *const ::std::os::raw::c_char, length: u32, alloc: ddwaf_allocator) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_is_array(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_is_bool(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_is_float(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_is_invalid(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_is_map(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_is_null(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_is_signed(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_is_string(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_is_unsigned(object: *const ddwaf_object) -> bool { false }
    pub unsafe fn ddwaf_object_set_array(object: *mut ddwaf_object, capacity: u16, alloc: ddwaf_allocator) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_bool(object: *mut ddwaf_object, value: bool) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_float(object: *mut ddwaf_object, value: f64) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_invalid(object: *mut ddwaf_object) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_map(object: *mut ddwaf_object, capacity: u16, alloc: ddwaf_allocator) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_null(object: *mut ddwaf_object) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_signed(object: *mut ddwaf_object, value: i64) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_string(object: *mut ddwaf_object, string: *const ::std::os::raw::c_char, length: u32, alloc: ddwaf_allocator) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_string_literal(object: *mut ddwaf_object, string: *const ::std::os::raw::c_char, length: u32) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_string_nocopy(object: *mut ddwaf_object, string: *const ::std::os::raw::c_char, length: u32) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_unsigned(object: *mut ddwaf_object, value: u64) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_set_log_cb(cb: ddwaf_log_cb, min_level: DDWAF_LOG_LEVEL) -> bool { false }
    pub unsafe fn ddwaf_subcontext_destroy(subcontext: ddwaf_subcontext) {}
    pub unsafe fn ddwaf_subcontext_eval(subcontext: ddwaf_subcontext, data: *mut ddwaf_object, alloc: ddwaf_allocator, result: *mut ddwaf_object, timeout: u64) -> DDWAF_RET_CODE { DDWAF_ERR_INTERNAL }
    pub unsafe fn ddwaf_subcontext_init(context: ddwaf_context) -> ddwaf_subcontext { std::ptr::null_mut() }
    pub unsafe fn ddwaf_synchronized_pool_allocator_init() -> ddwaf_allocator { std::ptr::null_mut() }
    pub unsafe fn ddwaf_unsynchronized_pool_allocator_init() -> ddwaf_allocator { std::ptr::null_mut() }
    pub unsafe fn ddwaf_user_allocator_init(alloc_fn: ddwaf_alloc_fn_type, free_fn: ddwaf_free_fn_type, udata: *mut ::std::os::raw::c_void, udata_free_fn: ddwaf_udata_free_fn_type) -> ddwaf_allocator { std::ptr::null_mut() }
}
