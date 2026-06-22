#![crate_type = "dylib"]
#![deny(clippy::correctness, clippy::perf, clippy::style, clippy::suspicious)]
#![allow(unused)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(unsafe_op_in_unsafe_fn)] // Bindgen generates some offending code...

use std::alloc::Layout;
use std::ptr::null;
use std::slice;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(feature = "dynamic")]
mod dylib;
#[cfg(feature = "dynamic")]
pub use dylib::*;

#[cfg(feature = "dynamic-external")]
mod dylib_external;
#[cfg(feature = "dynamic-external")]
pub use dylib_external::*;

// Implement [Send] and [Sync] for [ddwaf_object]. There is nothing thread unsafe about these unless
// its pointers are dereferences, which is inherently unsafe anyway.
unsafe impl Send for ddwaf_object {}
unsafe impl Sync for ddwaf_object {}

#[warn(clippy::pedantic)]
impl ddwaf_object {
    /// Drops the array data associated with the receiving [`ddwaf_object`].
    ///
    /// # Safety
    /// - The [`ddwaf_object`] must be a valid representation of an array.
    /// - The array must be an [`std::alloc::alloc`]ated array of [`ddwaf_object`] of the proper size.
    /// - The individual elements of the array must be valid [`ddwaf_object`]s that can be dropped
    ///   with [`ddwaf_object::drop_object`].
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn drop_array(&mut self) {
        debug_assert_eq!(self.obj_type(), DDWAF_OBJ_ARRAY);
        let array = unsafe { self.via.array };
        if array.capacity == 0 {
            return;
        }
        for i in 0..array.size {
            // We don't support 16-bit isize, so the unsigned -> signed cast is safe.
            #[allow(clippy::cast_possible_wrap)]
            let elem = unsafe { &mut *array.ptr.offset(i as isize) };
            unsafe { elem.drop_object() };
        }
        // Could only panic if sizeof(ddwaf_object) * capacity > isize::MAX, which can't happen
        // given that capacity is limited to u16::MAX.
        let layout = Layout::array::<ddwaf_object>(array.capacity as usize).unwrap();
        unsafe { std::alloc::dealloc(array.ptr.cast(), layout) };
    }

    /// Drops the map data associated with the receiving [`ddwaf_object`].
    ///
    /// # Safety
    /// - The [`ddwaf_object`] must be a valid representation of a map.
    /// - The map must be an [`std::alloc::alloc`]ated array of [`ddwaf_object`] of the proper size.
    /// - The individual elements of the map must be valid [`ddwaf_object`]s that can be dropped with
    ///   both [`ddwaf_object::drop_object`] and [`ddwaf_object::drop_key`].
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn drop_map(&mut self) {
        debug_assert_eq!(self.obj_type(), DDWAF_OBJ_MAP);
        let map = unsafe { self.via.map };
        if map.capacity == 0 {
            return;
        }
        for i in 0..map.size {
            #[allow(clippy::cast_possible_wrap)]
            let elem = unsafe { &mut *map.ptr.offset(i as isize) };
            unsafe { elem.key.drop_object() };
            unsafe { elem.val.drop_object() };
        }
        let layout = Layout::array::<_ddwaf_object_kv>(map.capacity as usize).unwrap();
        unsafe { std::alloc::dealloc(map.ptr.cast(), layout) };
    }

    /// Drops the value associated with the receiving [`ddwaf_object`].
    ///
    /// # Safety
    /// If the [`ddwaf_object`] is a string, array, or map, the respective requirements of the
    /// [`ddwaf_object::drop_string`], [`ddwaf_object::drop_array`], or [`ddwaf_object::drop_map`]
    /// methods apply.
    /// The method can't be called more than once.
    pub unsafe fn drop_object(&mut self) {
        match self.obj_type() {
            DDWAF_OBJ_STRING => unsafe { self.drop_string() },
            DDWAF_OBJ_ARRAY => unsafe { self.drop_array() },
            DDWAF_OBJ_MAP => unsafe { self.drop_map() },
            _ => { /* nothing to do */ }
        }
    }

    /// Drops the regular string associated with the receiving [`ddwaf_object`].
    ///
    /// # Safety
    /// - The [`ddwaf_object`] must be a valid representation of a string
    /// - The [`_ddwaf_object__bindgen_ty_1::str_`] field must have a
    ///   [`_ddwaf_object_string::ptr`] set from an allocation of `c_char` of the
    ///   size indicated by the [`_ddwaf_object_string::size`] field done with [`std::alloc::alloc`].
    #[allow(clippy::missing_panics_doc)]
    pub unsafe fn drop_string(&mut self) {
        debug_assert_eq!(self.obj_type(), DDWAF_OBJ_STRING);
        let sval = unsafe { self.via.str_.ptr };
        if sval.is_null() {
            return;
        }
        unsafe {
            std::alloc::dealloc(
                sval.cast(),
                Layout::array::<::std::os::raw::c_char>(self.via.str_.size as usize).unwrap(),
            );
        }
    }

    /// Returns the type of the [`ddwaf_object`]
    #[must_use]
    pub fn obj_type(&self) -> DDWAF_OBJ_TYPE {
        DDWAF_OBJ_TYPE::from(unsafe { self.type_ })
    }

    /// Returns true if the [`ddwaf_object`] is a string.
    #[must_use]
    pub fn is_string(&self) -> bool {
        (self.obj_type() & DDWAF_OBJ_STRING) != 0
    }

    /// Returns a slice of the bytes from the string associated with the receiving [`ddwaf_object`].
    ///
    /// # Safety
    /// - The [`ddwaf_object`] must be a valid representation of a string.
    unsafe fn string_vec(&self) -> &[u8] {
        debug_assert!(self.is_string());

        if self.obj_type() == DDWAF_OBJ_STRING || self.obj_type() == DDWAF_OBJ_LITERAL_STRING {
            let str = unsafe { self.via.str_ };
            if str.size == 0 {
                return &[];
            }
            unsafe { slice::from_raw_parts(str.ptr.cast(), str.size as usize) }
        } else {
            let sstr = unsafe { &self.via.sstr };
            let data = &sstr.data[..sstr.size as usize];
            // reinterpret &[i8] as &[u8]
            unsafe { std::slice::from_raw_parts(data.as_ptr().cast(), data.len()) }
        }
    }
}

impl std::cmp::PartialEq<ddwaf_object> for ddwaf_object {
    fn eq(&self, other: &ddwaf_object) -> bool {
        if self.is_string() && other.is_string() {
            let left = unsafe { self.string_vec() };
            let right = unsafe { other.string_vec() };
            return left == right;
        }

        if unsafe { self.type_ != other.type_ } {
            return false;
        }
        match self.obj_type() {
            DDWAF_OBJ_INVALID | DDWAF_OBJ_NULL => true,
            DDWAF_OBJ_SIGNED => unsafe { self.via.i64_.val == other.via.i64_.val },
            DDWAF_OBJ_UNSIGNED => unsafe { self.via.u64_.val == other.via.u64_.val },
            DDWAF_OBJ_BOOL => unsafe { self.via.b8.val == other.via.b8.val },
            DDWAF_OBJ_FLOAT => unsafe { self.via.f64_.val == other.via.f64_.val },
            DDWAF_OBJ_ARRAY => {
                let left = unsafe { self.via.array };
                let right = unsafe { other.via.array };
                if left.size != right.size {
                    return false;
                }
                if left.size == 0 {
                    return true;
                }
                for i in 0..left.size {
                    let left = unsafe { &*left.ptr.offset(i as isize) };
                    let right = unsafe { &*right.ptr.offset(i as isize) };
                    if left != right {
                        return false;
                    }
                }
                true
            }
            DDWAF_OBJ_MAP => {
                let left = unsafe { self.via.map };
                let right = unsafe { other.via.map };
                if left.size != right.size {
                    return false;
                }
                if left.size == 0 {
                    return true;
                }
                for i in 0..left.size {
                    let left = unsafe { &*left.ptr.offset(i as isize) };
                    let right = unsafe { &*right.ptr.offset(i as isize) };
                    if left.key != right.key || left.val != right.val {
                        return false;
                    }
                }
                true
            }

            _ => false,
        }
    }
}
impl std::fmt::Debug for ddwaf_object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("ddwaf_object");
        match self.obj_type() {
            DDWAF_OBJ_BOOL => dbg
                .field("type", &stringify!(DDWAF_OBJ_BOOL))
                .field("boolean", unsafe { &self.via.b8.val }),
            DDWAF_OBJ_FLOAT => dbg
                .field("type", &stringify!(DDWAF_OBJ_FLOAT))
                .field("f64", unsafe { &self.via.f64_.val }),
            DDWAF_OBJ_SIGNED => dbg
                .field("type", &stringify!(DDWAF_OBJ_SIGNED))
                .field("int", unsafe { &self.via.i64_.val }),
            DDWAF_OBJ_UNSIGNED => dbg
                .field("type", &stringify!(DDWAF_OBJ_UNSIGNED))
                .field("uint", unsafe { &self.via.u64_.val }),
            DDWAF_OBJ_STRING | DDWAF_OBJ_LITERAL_STRING => {
                let sval = unsafe { self.string_vec() };
                let sval = String::from_utf8_lossy(sval);
                dbg.field(
                    "type",
                    if self.obj_type() == DDWAF_OBJ_STRING {
                        &stringify!(DDWAF_OBJ_STRING)
                    } else {
                        &stringify!(DDWAF_OBJ_LITERAL_STRING)
                    },
                )
                .field("string", &sval)
            }
            DDWAF_OBJ_SMALL_STRING => {
                let sval = unsafe { self.string_vec() };
                let sval = String::from_utf8_lossy(sval);
                dbg.field("type", &stringify!(DDWAF_OBJ_SMALL_STRING))
                    .field("string", &sval)
            }
            DDWAF_OBJ_ARRAY => {
                let array = unsafe { self.via.array };
                let array: &[ddwaf_object] =
                    unsafe { slice::from_raw_parts(array.ptr.cast(), array.size as usize) };
                dbg.field("type", &stringify!(DDWAF_OBJ_ARRAY))
                    .field("array", &array)
            }
            DDWAF_OBJ_MAP => {
                let map = unsafe { self.via.map };
                let map: &[_ddwaf_object_kv] =
                    unsafe { slice::from_raw_parts(map.ptr.cast(), map.size as usize) };
                dbg.field("type", &stringify!(DDWAF_OBJ_MAP))
                    .field("map", &map)
            }
            DDWAF_OBJ_NULL => dbg.field("type", &stringify!(DDWAF_OBJ_NULL)),
            DDWAF_OBJ_INVALID => dbg.field("type", &stringify!(DDWAF_OBJ_INVALID)),
            unknown => dbg.field("type", &unknown),
        };

        dbg.finish_non_exhaustive()
    }
}

impl std::fmt::Debug for _ddwaf_object_kv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("ddwaf_object_kv");
        dbg.field("key", &self.key)
            .field("val", &self.val)
            .finish_non_exhaustive()
    }
}
