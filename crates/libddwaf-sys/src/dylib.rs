#![allow(clippy::missing_safety_doc)]

use std::io::{copy, Write};

use crate::*;

use lazy_static::lazy_static;

#[cold]
fn null_library() -> libloading::Library {
    #[cfg(unix)]
    return unsafe { libloading::os::unix::Library::from_raw(std::ptr::null_mut()) }.into();
    #[cfg(windows)]
    return unsafe { libloading::os::windows::Library::from_raw(0) }.into();
    #[cfg(not(any(unix, windows)))]
    panic!("unsupported platform for dynamic feature");
}

const LIBDDWAF_SHARED_OBJECT: &[u8] = include_bytes!(env!("LIBDDWAF_SHARED_OBJECT.zst"));

lazy_static! {
    static ref LIBRARY: ddwaf = init().unwrap_or_default();
}

/// Initialize the global shared library instance.
///
/// Dumps the shared object blob to a temporary file, then proceeds to load it
/// with the [ddwaf::new].
fn init() -> Option<ddwaf> {
    tracing::debug!("dumping embedded libddwaf shared object to a temporary file...");
    let mut tmp = match tempfile::NamedTempFile::new() {
        Ok(tmp) => tmp,
        Err(e) => {
            tracing::error!("failed to create temporary file: {e}");
            return None;
        }
    };

    let mut decoder = match zstd::Decoder::new(LIBDDWAF_SHARED_OBJECT) {
        Ok(dec) => dec,
        Err(e) => {
            tracing::error!("failed to create zstd decoder: {e}");
            return None;
        }
    };

    if let Err(e) = copy(&mut decoder, &mut tmp) {
        eprintln!("failed to write libddwaf shared object to temporay file: {e}");
        tracing::error!("failed to write libddwaf shared object to temporay file: {e}");
        return None;
    }
    if let Err(e) = tmp.flush() {
        tracing::error!("failed to flush libddwaf shared object to temporay file: {e}");
        return None;
    }

    // into_temp_path() closes the write handle (required on Windows: LoadLibraryExW cannot
    // open a file while an existing write handle is open ÔÇö ERROR_SHARING_VIOLATION) while
    // keeping the RAII deletion semantics of NamedTempFile. This makes cleanup panic-safe:
    // if ddwaf::new() panics, the TempPath Drop runs and removes the file.
    let path = tmp.into_temp_path();

    tracing::debug!("loading libddwaf shared object from temporary file {}", path.display());
    let result = unsafe { ddwaf::new(&*path) };
    // TempPath::drop() attempts deletion here.
    // On Unix: succeeds ÔÇö dlopen holds the inode open via its own fd, so the file data
    //   persists until the library is unloaded.
    // On Windows: fails silently ÔÇö the OS holds a mandatory file lock on loaded DLLs.
    //   The file persists until OS temp cleanup since lazy_static never drops LIBRARY.
    match result {
        Ok(lib) => Some(lib),
        Err(e) => {
            tracing::error!("failed to load libddwaf shared object: {e}");
            None
        }
    }
}

/// Re-exports a function from the static [`ddwaf``] instance, so that the API
/// remains consistent with the one when the `dynamic` feature is not enabled.
/// All exported functions will be tagged as `extern "C"`, and the body provided
/// in the macro corresponds to the default value to return when the shared
/// library could not be loaded.
macro_rules! reexport {
    (
        $($(#[$meta:meta])* $vis:vis unsafe fn $name:ident($($arg_name:ident: $arg_type: ty),*) $(-> $ret_type:ty)? { $($fallback:expr)? })*
    ) => {
        $(
            $(#[$meta])*
            $vis unsafe extern "C" fn $name($($arg_name: $arg_type),*) $(-> $ret_type)? {
                unsafe { LIBRARY.$name($($arg_name),*) }
            }
        )*

        impl Default for ddwaf {
            #[cold]
            fn default() -> Self {
                $(
                    $(#[$meta])*
                    #[cold]
                    unsafe extern "C" fn $name($($arg_name: $arg_type),*) $(-> $ret_type)? {$($fallback)?}
                )*

                Self {
                    __library: null_library(),
                    $($(#[$meta])* $name,)*
                }
            }
        }
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
    pub unsafe fn ddwaf_context_multieval(context: ddwaf_context, data: *mut ddwaf_object, alloc: ddwaf_allocator, result: *mut ddwaf_object, timeout: u64) -> DDWAF_RET_CODE { DDWAF_ERR_INTERNAL }
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
    // Not exported from the Windows DLL; excluded from dynamic bindings on Windows.
    #[cfg(not(windows))] pub unsafe fn ddwaf_object_set_string_nocopy(object: *mut ddwaf_object, string: *const ::std::os::raw::c_char, length: u32) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_object_set_unsigned(object: *mut ddwaf_object, value: u64) -> *mut ddwaf_object { std::ptr::null_mut() }
    pub unsafe fn ddwaf_set_log_cb(cb: ddwaf_log_cb, min_level: DDWAF_LOG_LEVEL) -> bool { false }
    pub unsafe fn ddwaf_subcontext_destroy(subcontext: ddwaf_subcontext) {}
    pub unsafe fn ddwaf_subcontext_eval(subcontext: ddwaf_subcontext, data: *mut ddwaf_object, alloc: ddwaf_allocator, result: *mut ddwaf_object, timeout: u64) -> DDWAF_RET_CODE { DDWAF_ERR_INTERNAL }
    pub unsafe fn ddwaf_subcontext_init(context: ddwaf_context) -> ddwaf_subcontext { std::ptr::null_mut() }
    pub unsafe fn ddwaf_subcontext_multieval(subcontext: ddwaf_subcontext, data: *mut ddwaf_object, alloc: ddwaf_allocator, result: *mut ddwaf_object, timeout: u64) -> DDWAF_RET_CODE { DDWAF_ERR_INTERNAL }
    pub unsafe fn ddwaf_synchronized_pool_allocator_init() -> ddwaf_allocator { std::ptr::null_mut() }
    pub unsafe fn ddwaf_unsynchronized_pool_allocator_init() -> ddwaf_allocator { std::ptr::null_mut() }
    pub unsafe fn ddwaf_user_allocator_init(alloc_fn: ddwaf_alloc_fn_type, free_fn: ddwaf_free_fn_type, udata: *mut ::std::os::raw::c_void, udata_free_fn: ddwaf_udata_free_fn_type) -> ddwaf_allocator { std::ptr::null_mut() }
}
