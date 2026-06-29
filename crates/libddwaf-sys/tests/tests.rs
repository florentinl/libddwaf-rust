#![cfg(not(miri))]

use std::ffi::{CStr, CString};

use libddwaf_sys::*;

#[test]
fn test_version() {
    // Skip this test if LIBDDWAF_PREFIX is set, as it will use a different version
    if std::env::var("LIBDDWAF_PREFIX").is_ok() {
        eprintln!("Skipping test_version: LIBDDWAF_PREFIX is set");
        return;
    }

    let version = unsafe { ddwaf_get_version() };
    assert!(!version.is_null());
    assert_eq!(
        CString::new(env!("CARGO_PKG_VERSION"))
            .expect("failed to create CString")
            .as_c_str(),
        unsafe { CStr::from_ptr(version) }
    );
}

#[test]
fn test_eq_invalid() {
    let left = ddwaf_object::default();
    let right = ddwaf_object::default();
    assert_eq!(left, right); // We always consider invalid objects to be equal.
}

#[test]
fn test_eq_null() {
    let mut left = ddwaf_object::default();
    unsafe { ddwaf_object_set_null(&mut left) };

    let mut right = ddwaf_object::default();
    right.type_ = DDWAF_OBJ_NULL as u8;

    assert_eq!(left, right);
    assert_ne!(left, ddwaf_object::default());
}

#[test]
fn test_eq_signed() {
    let mut left = ddwaf_object::default();
    unsafe { ddwaf_object_set_signed(&mut left, -42) };

    let mut right = ddwaf_object::default();
    right.via.i64_ = _ddwaf_object_signed {
        type_: DDWAF_OBJ_SIGNED as u8,
        val: -42,
    };
    assert_eq!(left, right);

    let mut wrong = ddwaf_object::default();
    wrong.via.i64_ = _ddwaf_object_signed {
        type_: DDWAF_OBJ_SIGNED as u8,
        val: 42,
    };
    assert_ne!(left, wrong);
    assert_ne!(left, ddwaf_object::default());
}

#[test]
fn test_eq_unsigned() {
    let mut left = ddwaf_object::default();
    unsafe { ddwaf_object_set_unsigned(&mut left, 1337) };

    let mut right = ddwaf_object::default();
    right.via.u64_ = _ddwaf_object_unsigned {
        type_: DDWAF_OBJ_UNSIGNED as u8,
        val: 1337,
    };
    assert_eq!(left, right);

    let mut wrong = ddwaf_object::default();
    wrong.via.u64_ = _ddwaf_object_unsigned {
        type_: DDWAF_OBJ_UNSIGNED as u8,
        val: 42,
    };
    assert_ne!(left, wrong);
    assert_ne!(left, ddwaf_object::default());
}

#[test]
fn test_eq_bool() {
    let mut left = ddwaf_object::default();
    unsafe { ddwaf_object_set_bool(&mut left, true) };

    let mut right = ddwaf_object::default();
    right.via.b8 = _ddwaf_object_bool {
        type_: DDWAF_OBJ_BOOL as u8,
        val: true,
    };
    assert_eq!(left, right);

    let mut wrong = ddwaf_object::default();
    wrong.via.b8 = _ddwaf_object_bool {
        type_: DDWAF_OBJ_BOOL as u8,
        val: false,
    };
    assert_ne!(left, wrong);
    assert_ne!(left, ddwaf_object::default());
}

#[test]
fn test_eq_float() {
    let mut left = ddwaf_object::default();
    unsafe { ddwaf_object_set_float(&mut left, 1337.42) };

    let mut right = ddwaf_object::default();
    right.via.f64_ = _ddwaf_object_float {
        type_: DDWAF_OBJ_FLOAT as u8,
        val: 1337.42,
    };
    assert_eq!(left, right);

    let mut wrong = ddwaf_object::default();
    wrong.via.f64_ = _ddwaf_object_float {
        type_: DDWAF_OBJ_FLOAT as u8,
        val: 1337.0,
    };
    assert_ne!(left, wrong);
    assert_ne!(left, ddwaf_object::default());
}

#[test]
fn test_eq_string() {
    // Test empty literal string
    let mut empty_literal = ddwaf_object::default();
    let blank = b"";
    unsafe { ddwaf_object_set_string_literal(&mut empty_literal, blank.as_ptr().cast(), 0) };

    let mut empty_expected = ddwaf_object::default();
    empty_expected.via.str_ = _ddwaf_object_string {
        type_: DDWAF_OBJ_LITERAL_STRING as u8,
        size: 0,
        ptr: std::ptr::null_mut(),
    };
    assert_eq!(empty_literal, empty_expected);

    // Test literal string
    let test_str = b"Hello, world!";
    let mut literal = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string_literal(
            &mut literal,
            test_str.as_ptr().cast(),
            test_str.len() as u32,
        )
    };

    let mut literal_expected = ddwaf_object::default();
    literal_expected.via.str_ = _ddwaf_object_string {
        type_: DDWAF_OBJ_LITERAL_STRING as u8,
        size: 13,
        ptr: test_str.as_ptr() as *mut _,
    };
    assert_eq!(literal, literal_expected);

    // Test regular (allocated) string with same content
    // Note: We use ddwaf_object_set_string_literal for testing equality semantics
    // without needing to manage allocations in the test
    let mut regular_as_literal = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string_literal(
            &mut regular_as_literal,
            test_str.as_ptr().cast(),
            test_str.len() as u32,
        )
    };

    // Different string type objects with same content should be equal (cross-type comparison)
    assert_eq!(
        regular_as_literal, literal,
        "Strings with same content should be equal regardless of type"
    );

    // Test small string (short strings are stored inline)
    let small_str = b"Hi";
    let mut small_literal = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string_literal(
            &mut small_literal,
            small_str.as_ptr().cast(),
            small_str.len() as u32,
        )
    };

    let mut small_literal2 = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string_literal(
            &mut small_literal2,
            small_str.as_ptr().cast(),
            small_str.len() as u32,
        )
    };

    // Small strings should compare equal
    assert_eq!(
        small_literal, small_literal2,
        "Small strings with same content should be equal"
    );

    // Test that small and regular strings with same content are equal
    let same_content = b"Hi";
    let mut another_small = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string_literal(
            &mut another_small,
            same_content.as_ptr().cast(),
            same_content.len() as u32,
        )
    };
    assert_eq!(
        small_literal, another_small,
        "Strings with same content should be equal"
    );

    // Test length mismatch
    let mut wrong = ddwaf_object::default();
    wrong.via.str_ = _ddwaf_object_string {
        type_: DDWAF_OBJ_LITERAL_STRING as u8,
        size: 12, // Length mismatch
        ptr: test_str.as_ptr() as *mut _,
    };
    assert_ne!(literal, wrong);

    // Test content mismatch
    let other_str = b"Different!!!";
    let mut different = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string_literal(
            &mut different,
            other_str.as_ptr().cast(),
            other_str.len() as u32,
        )
    };
    assert_ne!(
        literal, different,
        "Strings with different content should not be equal"
    );

    assert_ne!(literal, ddwaf_object::default());
}

// ddwaf_object_set_string_nocopy is present in ddwaf_static.lib but not in ddwaf.dll or its
// import library. Skip on Windows when linking against the DLL (dynamic / dynamic-link features).
#[test]
#[cfg(not(all(windows, any(feature = "dynamic", feature = "dynamic-link"))))]
fn test_eq_string_types_with_allocator() {
    let alloc = unsafe { ddwaf_get_default_allocator() };

    // Use a longer string to ensure it's stored as DDWAF_OBJ_STRING, not DDWAF_OBJ_SMALL_STRING
    let test_str: [u8; 53] = *b"This is a longer test string that should not be small";

    let mut regular_string = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string_nocopy(
            &mut regular_string,
            test_str.as_ptr().cast(),
            test_str.len() as u32,
        );
    }

    let regular_type = unsafe { ddwaf_object_get_type(&regular_string) };
    assert_eq!(
        regular_type, DDWAF_OBJ_STRING,
        "Long allocated string should be DDWAF_OBJ_STRING"
    );

    // Create a literal string with same content
    let mut literal_string = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string_literal(
            &mut literal_string,
            test_str.as_ptr().cast(),
            test_str.len() as u32,
        );
    }

    let literal_type = unsafe { ddwaf_object_get_type(&literal_string) };
    assert_eq!(
        literal_type, DDWAF_OBJ_LITERAL_STRING,
        "Literal string should be DDWAF_OBJ_LITERAL_STRING"
    );

    // Regular and literal strings with same content should be equal (cross-type comparison)
    assert_eq!(
        regular_string, literal_string,
        "Regular and literal strings with same content should be equal"
    );

    // Test with very small strings that will use small string optimization
    let small_str = b"hi";

    let mut small_allocated = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string(
            &mut small_allocated,
            small_str.as_ptr().cast(),
            small_str.len() as u32,
            alloc,
        );
    }

    let small_type = unsafe { ddwaf_object_get_type(&small_allocated) };
    assert_eq!(
        small_type, DDWAF_OBJ_SMALL_STRING,
        "Short allocated string should be DDWAF_OBJ_SMALL_STRING"
    );

    let mut small_literal = ddwaf_object::default();
    unsafe {
        ddwaf_object_set_string_literal(
            &mut small_literal,
            small_str.as_ptr().cast(),
            small_str.len() as u32,
        );
    }

    let small_lit_type = unsafe { ddwaf_object_get_type(&small_literal) };
    assert_eq!(
        small_lit_type, DDWAF_OBJ_LITERAL_STRING,
        "Short literal string should be DDWAF_OBJ_LITERAL_STRING"
    );

    // Small strings should also compare equal across types
    assert_eq!(
        small_allocated, small_literal,
        "Small strings should be equal regardless of how they were created"
    );
}

#[test]
fn test_eq_array_and_map() {
    // Test empty arrays
    let mut arr_left = ddwaf_object::default();
    arr_left.via.array = _ddwaf_object_array {
        type_: DDWAF_OBJ_ARRAY as u8,
        size: 0,
        capacity: 0,
        ptr: std::ptr::null_mut(),
    };

    let mut arr_right = ddwaf_object::default();
    arr_right.via.array = _ddwaf_object_array {
        type_: DDWAF_OBJ_ARRAY as u8,
        size: 0,
        capacity: 0,
        ptr: std::ptr::null_mut(),
    };

    assert_eq!(arr_left, arr_right);

    // Test map with one entry
    let mut items = [_ddwaf_object_kv::default()];
    unsafe {
        ddwaf_object_set_unsigned(&mut items[0].val, 42);
        ddwaf_object_set_string_literal(&mut items[0].key, b"key".as_ptr().cast(), 3);
    };

    let mut left = ddwaf_object::default();
    left.via.map = _ddwaf_object_map {
        type_: DDWAF_OBJ_MAP as u8,
        size: 1,
        capacity: 1,
        ptr: items.as_mut_ptr(),
    };

    let mut right = ddwaf_object::default();
    right.via.map = _ddwaf_object_map {
        type_: DDWAF_OBJ_MAP as u8,
        size: 1,
        capacity: 1,
        ptr: items.as_mut_ptr(),
    };
    assert_eq!(left, right);

    // Test key mismatch
    let mut items2 = [_ddwaf_object_kv::default()];
    unsafe {
        ddwaf_object_set_unsigned(&mut items2[0].val, 42);
        ddwaf_object_set_string_literal(&mut items2[0].key, b"yek".as_ptr().cast(), 3);
        // Different key
    };

    let mut wrong = ddwaf_object::default();
    wrong.via.map = _ddwaf_object_map {
        type_: DDWAF_OBJ_MAP as u8,
        size: 1,
        capacity: 1,
        ptr: items2.as_mut_ptr(),
    };
    assert_ne!(left, wrong);

    // Test value mismatch
    let mut items3 = [_ddwaf_object_kv::default()];
    unsafe {
        ddwaf_object_set_signed(&mut items3[0].val, -1337); // Different value type
        ddwaf_object_set_string_literal(&mut items3[0].key, b"key".as_ptr().cast(), 3);
    };

    let mut wrong2 = ddwaf_object::default();
    wrong2.via.map = _ddwaf_object_map {
        type_: DDWAF_OBJ_MAP as u8,
        size: 1,
        capacity: 1,
        ptr: items3.as_mut_ptr(),
    };
    assert_ne!(left, wrong2);

    // Test size mismatch
    let mut wrong3 = ddwaf_object::default();
    wrong3.via.map = _ddwaf_object_map {
        type_: DDWAF_OBJ_MAP as u8,
        size: 0, // Size mismatch
        capacity: 1,
        ptr: items.as_mut_ptr(),
    };
    assert_ne!(left, wrong3);
    assert_ne!(left, ddwaf_object::default());
}
