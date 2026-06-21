use libddwaf::{object::*, waf_array, waf_map, waf_object};

#[test]
#[allow(clippy::float_cmp)] // No operations are done on the values, they should be the same.
fn defaults() {
    let obj = WafObject::default();
    assert!(!obj.is_valid());
    assert_eq!(obj.object_type(), WafObjectType::Invalid);

    let obj = WafSigned::default();
    assert!(obj.is_valid());
    assert_eq!(obj.value(), 0);

    let obj = WafUnsigned::default();
    assert!(obj.is_valid());
    assert_eq!(obj.value(), 0);

    let obj = WafString::default();
    assert!(obj.is_valid());
    assert_eq!(obj.as_str(), Ok(""));

    let obj = WafArray::default();
    assert!(obj.is_valid());
    assert_eq!(obj.len(), 0);

    let obj = WafMap::default();
    assert!(obj.is_valid());
    assert_eq!(obj.len(), 0);

    let obj = WafBool::default();
    assert!(obj.is_valid());
    assert!(!obj.value());

    let obj = WafFloat::default();
    assert!(obj.is_valid());
    assert_eq!(obj.value(), 0.0);

    let obj = WafNull::default();
    assert!(obj.is_valid());
}

#[test]
fn test_eq() {
    let invalid = WafObject::default();
    let string = WafString::default();
    let bool = WafBool::default();
    let float = WafFloat::default();
    let signed = WafSigned::default();
    let unsigned = WafUnsigned::default();
    let array = WafArray::default();
    let map = WafMap::default();
    let null = WafNull::default();

    assert_eq!(invalid, invalid);
    assert_eq!(string, string);
    assert_eq!(bool, bool);
    assert_eq!(float, float);
    assert_eq!(signed, signed);
    assert_eq!(unsigned, unsigned);
    assert_eq!(array, array);
    assert_eq!(map, map);
    assert_eq!(null, null);

    assert_ne!(invalid, string);
    assert_ne!(invalid, bool);
    assert_ne!(invalid, float);
    assert_ne!(invalid, signed);
    assert_ne!(invalid, unsigned);
    assert_ne!(invalid, array);
    assert_ne!(invalid, map);

    assert_ne!(string, bool);
    assert_ne!(string, float);
    assert_ne!(string, signed);
    assert_ne!(string, unsigned);
    assert_ne!(string, array);
    assert_ne!(string, map);

    assert_ne!(bool, float);
    assert_ne!(bool, signed);
    assert_ne!(bool, unsigned);
    assert_ne!(bool, array);
    assert_ne!(bool, map);

    assert_ne!(float, signed);
    assert_ne!(float, unsigned);
    assert_ne!(float, array);
    assert_ne!(float, map);

    assert_ne!(signed, unsigned);
    assert_ne!(signed, array);
    assert_ne!(signed, map);

    assert_ne!(unsigned, array);
    assert_ne!(unsigned, map);

    assert_ne!(array, map);
}

#[test]
fn sample_mixed_object() {
    let mut root = WafArray::new(4);
    root[0] = 42_u64.into();
    root[1] = "Hello, world!".into();
    root[2] = WafArray::new(1).into();
    root[2].as_type_mut::<WafArray>().unwrap()[0] = 123_u64.into();

    let mut map = WafMap::new(7);
    map[0] = ("key 1", "value 1").into();
    map[1] = ("key 2", -2_i64).into();
    map[2] = ("key 3", 2_u32).into();
    map[3] = ("key 4", 5.2).into();
    map[4] = ("key 5", ()).into();
    map[5] = ("key 6", true).into();
    root[3] = map.into();

    let res = format!("{root:?}");
    assert_eq!(
        res,
        "WafArray[WafUnsigned(42), WafString(\"Hello, \
        world!\"), WafArray[WafUnsigned(123)], WafMap{\
        \"key 1\"=WafString(\"value 1\"), \"key 2\"=\
        WafSigned(-2), \"key 3\"=WafUnsigned(2), \
        \"key 4\"=WafFloat(5.2), \"key 5\"=WafNull, \
        \"key 6\"=WafBool(true), WafInvalid=WafInvalid}]"
    );
}

#[test]
fn sample_mixed_object_macro() {
    let root = waf_array!(
        42_u64,
        "Hello, world!",
        waf_array!(123_u64),
        waf_map!(
            ("key 1", "value 1"),
            ("key 2", -2_i64),
            ("key 3", 2_u64),
            ("key 4", 5.2),
            ("key 5", waf_object!(null)),
            ("key 6", waf_array!()),
            ("key 7", waf_array!(true, false)),
        ),
        waf_array!(),
        waf_map!(),
    );

    assert_eq!(
        format!("{root:?}"),
        "WafArray[WafUnsigned(42), WafString(\"Hello, \
        world!\"), WafArray[WafUnsigned(123)], WafMap{\
        \"key 1\"=WafString(\"value 1\"), \"key 2\"=\
        WafSigned(-2), \"key 3\"=WafUnsigned(2), \
        \"key 4\"=WafFloat(5.2), \"key 5\"=WafNull, \
        \"key 6\"=WafArray[], \"key 7\"=WafArray[WafBool(true), \
        WafBool(false)]}, WafArray[], WafMap{}]"
    );
}

#[test]
fn string_debug_value() {
    let obj = waf_map!((r#"key"hey"#, r"value\n"));
    assert_eq!(
        format!("{obj:?}"),
        r#"WafMap{"key\"hey"=WafString("value\\n")}"#
    );
}

#[test]
#[allow(clippy::float_cmp)] // No operations are done on the values, they should be the same.
fn ddwaf_obj_from_conversions() {
    let obj: WafObject = 42u64.into();
    assert_eq!(obj.to_u64().unwrap(), 42u64);
    assert_eq!(obj.to_i64().unwrap(), 42i64);

    let obj: WafObject = (-42i64).into();
    assert_eq!(obj.to_i64().unwrap(), -42i64);

    let obj: WafObject = 3.0.into();
    assert_eq!(obj.to_f64().unwrap(), 3.0f64);

    let obj: WafObject = true.into();
    assert!(obj.to_bool().unwrap());

    let obj: WafObject = ().into();
    assert_eq!(obj.object_type(), WafObjectType::Null);

    let obj: WafObject = "Hello, world!".into();
    assert_eq!(obj.to_str(), Some("Hello, world!"));

    let obj: WafObject = b"Hello, world!"[..].into();
    assert_eq!(obj.to_str(), Some("Hello, world!"));
}

#[test]
fn ddwaf_obj_failed_conversions() {
    let mut obj: WafObject = ().into();
    assert!(obj.as_type::<WafBool>().is_none());
    assert!(obj.as_type_mut::<WafBool>().is_none());

    assert!(obj.to_bool().is_none());
    assert!(obj.to_u64().is_none());
    assert!(obj.to_i64().is_none());
    assert!(obj.to_f64().is_none());
    assert!(obj.to_str().is_none());
}

#[test]
fn invalid_utf8() {
    let non_utf8_str: &[u8] = &[0x80];
    let obj: Keyed<WafString> = (non_utf8_str, non_utf8_str).into();
    assert_eq!(format!("{obj:?}"), r#""\x80"=WafString("\x80")"#);

    assert!(obj.key_str().is_err());
    assert!(obj.as_str().is_err());
}

#[test]
fn empty_key() {
    let map = waf_map!(("", 42_u64));
    let empty_slice: &[u8] = &[];
    assert_eq!(map[0].key_bytes().unwrap(), empty_slice);
}

#[test]
#[should_panic(expected = "index out of bounds (3 >= 3)")]
fn array_index_out_of_bounds() {
    let arr = waf_array!(1u64, "hello", waf_object!(null));
    let _ = arr[3]; // Panics
}

#[test]
#[should_panic(expected = "index out of bounds (3 >= 3)")]
fn array_index_mut_of_bounds() {
    let mut arr = waf_array!(1u64, "hello", waf_object!(null));
    arr[3] = 42u64.into(); // Panics
}

#[test]
#[should_panic(expected = "index out of bounds (3 >= 3)")]
fn map_index_out_of_bounds() {
    let arr = waf_map!(("a", 1u64), ("b", "hello"), ("c", waf_object!(null)));
    let _ = arr[3]; // Panics
}

#[test]
#[should_panic(expected = "index out of bounds (3 >= 3)")]
fn map_index_mut_of_bounds() {
    let mut arr = waf_map!(("a", 1u64), ("b", "hello"), ("c", waf_object!(null)));
    arr[3] = Keyed::from(("d", 42u64)); // Panics
}

#[test]
fn keyed_obj_methods() {
    let mut map = waf_map!(("key", 42_u64));
    let elem = &mut map[0];
    assert!(elem.as_type::<WafBool>().is_none());
    let elem_cast = elem.as_type::<WafUnsigned>().unwrap();
    assert_eq!(elem_cast.value().value(), 42u64);

    assert!(elem.as_type_mut::<WafBool>().is_none());

    let elem_cast = elem.as_type_mut::<WafUnsigned>().unwrap();
    assert_eq!(elem_cast.value().value(), 42u64);

    let key_mut = elem.key_mut();
    let _ = std::mem::replace(key_mut, "key 2".into());
    assert_eq!(elem.key_str().unwrap(), "key 2");
}

#[test]
fn map_fetching_methods() {
    let mut map = waf_map!(("key1", 1u64), ("key2", 2u64),);

    // index
    assert_eq!(map[0].key_bytes().unwrap(), b"key1");
    // index mut
    let key_mut = map[0].key_mut();
    let new_key = WafString::from(b"new key");
    let _ = std::mem::replace(key_mut, new_key.into());
    assert_eq!(map[0].key_bytes().unwrap(), b"new key");

    // get
    assert_eq!(map.get_bstr(b"key2").unwrap().to_u64().unwrap(), 2);
    assert!(map.get_bstr(b"bad key").is_none());
    // get_str
    assert_eq!(map.get_str("key2").unwrap().to_u64().unwrap(), 2);
    assert!(map.get_str("bad key").is_none());
    // get
    assert_eq!(
        map.get(WafString::new_literal(&b"key2"[..]))
            .unwrap()
            .to_u64()
            .unwrap(),
        2
    );

    // get_mut
    let key_mut = map.get_mut(b"key2").unwrap().key_mut();
    let new_key = WafString::from(b"key3");
    let _ = std::mem::replace(key_mut, new_key.into());
    let entry_k3 = map.get_str_mut("key3").unwrap();
    let new_entry: Keyed<WafUnsigned> = ("key3", 3u64).into();
    let _ = std::mem::replace(entry_k3, new_entry.into());
    assert_eq!(map.get_str("key3").unwrap().to_u64().unwrap(), 3);

    assert!(map.get_mut(b"bad key").is_none());

    // get_str_mut
    let key_mut = map.get_str_mut("key3").unwrap().key_mut();
    let new_key = WafString::from(b"key4");
    let _ = std::mem::replace(key_mut, new_key.into());
    assert_eq!(map.get_str("key4").unwrap().to_u64().unwrap(), 3);

    assert!(map.get_str_mut("bad key").is_none());
}

#[test]
fn array_iteration() {
    let mut arr = waf_array!(1u64, "foo", waf_array!("xyz"), waf_object!(null));

    for (i, elem) in arr.iter().enumerate() {
        match i {
            0 => assert_eq!(elem.to_u64().unwrap(), 1),
            1 => assert_eq!(elem.to_str().unwrap(), "foo"),
            2 => assert_eq!(elem.as_type::<WafArray>().unwrap().len(), 1),
            3 => assert_eq!(elem.object_type(), WafObjectType::Null),
            _ => unreachable!(),
        }
    }

    for (i, elem) in arr.iter_mut().enumerate() {
        match i {
            0 => assert_eq!(elem.to_u64().unwrap(), 1),
            1 => {
                assert_eq!(elem.to_str().unwrap(), "foo");
                let new_str: WafString = "bar".into();
                let _ = std::mem::replace(elem, new_str.into());
            }
            2 => assert_eq!(elem.as_type::<WafArray>().unwrap().len(), 1),
            3 => assert_eq!(elem.object_type(), WafObjectType::Null),
            _ => unreachable!(),
        }
    }
    assert_eq!(arr[1].to_str().unwrap(), "bar");

    for (i, elem) in arr.into_iter().enumerate() {
        match i {
            0 => assert_eq!(elem.to_u64().unwrap(), 1),
            1 => assert_eq!(elem.to_str().unwrap(), "bar"),
            2 => assert_eq!(elem.as_type::<WafArray>().unwrap().len(), 1),
            3 => assert_eq!(elem.object_type(), WafObjectType::Null),
            _ => unreachable!(),
        }
    }
}

#[test]
fn map_iteration() {
    let mut map = waf_map!(
        ("key1", 1u64),
        ("key2", "foo"),
        ("key3", waf_array!("xyz")),
        ("key4", waf_object!(null))
    );

    for (i, elem) in map.iter().enumerate() {
        match i {
            0 => {
                assert_eq!(elem.key_str().unwrap(), "key1");
                assert_eq!(elem.to_u64().unwrap(), 1);
            }
            1 => {
                assert_eq!(elem.key_str().unwrap(), "key2");
                assert_eq!(elem.to_str().unwrap(), "foo");
            }
            2 => {
                assert_eq!(elem.key_str().unwrap(), "key3");
                assert_eq!(elem.as_type::<WafArray>().unwrap().len(), 1);
            }
            3 => {
                assert_eq!(elem.key_str().unwrap(), "key4");
                assert_eq!(elem.object_type(), WafObjectType::Null);
            }
            _ => unreachable!(),
        }
    }

    for (i, elem) in map.iter_mut().enumerate() {
        match i {
            0 => assert_eq!(elem.to_u64().unwrap(), 1),
            1 => {
                assert_eq!(elem.key_str().unwrap(), "key2");
                assert_eq!(elem.to_str().unwrap(), "foo");
                let new_val: Keyed<WafString> = ("new_key", "bar").into();
                let _ = std::mem::replace(elem, new_val.into());
            }
            2 => assert_eq!(elem.key_str().unwrap(), "key3"),
            3 => assert_eq!(elem.key_str().unwrap(), "key4"),
            _ => unreachable!(),
        }
    }

    assert_eq!(map[1].key_str().unwrap(), "new_key");
    assert_eq!(map[1].to_str().unwrap(), "bar");

    for (i, elem) in map.into_iter().enumerate() {
        match i {
            0 => assert_eq!(elem.key_str().unwrap(), "key1"),
            1 => assert_eq!(elem.key_str().unwrap(), "new_key"),
            2 => assert_eq!(elem.key_str().unwrap(), "key3"),
            3 => assert_eq!(elem.key_str().unwrap(), "key4"),
            _ => unreachable!(),
        }
    }
}

#[test]
fn partial_iteration() {
    let arr = waf_array!(1u64, "foo");
    for elem in arr {
        if elem.object_type() == WafObjectType::Unsigned {
            break;
        }
    }

    let map = waf_map!(("key1", 1u64), ("key2", "foo"));
    for elem in map {
        if elem.object_type() == WafObjectType::Unsigned {
            break;
        }
    }
}

#[test]
fn iteration_of_empty_containers() {
    let mut arr: WafArray = waf_array!();
    assert!(arr.iter().next().is_none());
    assert!(arr.iter_mut().next().is_none());
    assert!(arr.into_iter().next().is_none());

    let mut map = waf_map!();
    assert!(map.iter().next().is_none());
    assert!(map.iter_mut().next().is_none());
    assert!(map.into_iter().next().is_none());
}

#[test]
fn iteration_of_keyed_array() {
    let mut map = waf_map!(("key1", waf_array!(1u64, "foo")));
    let keyed_array: &mut Keyed<WafArray> = map[0].as_type_mut().unwrap();

    for (i, elem) in keyed_array.iter().enumerate() {
        match i {
            0 => assert_eq!(elem.to_u64().unwrap(), 1),
            1 => assert_eq!(elem.to_str().unwrap(), "foo"),
            _ => unreachable!(),
        }
    }

    for (i, elem) in keyed_array.iter_mut().enumerate() {
        match i {
            0 => assert_eq!(elem.to_u64().unwrap(), 1),
            1 => {
                assert_eq!(elem.to_str().unwrap(), "foo");
                let new_str: WafString = "bar".into();
                let _ = std::mem::replace(elem, new_str.into());
            }
            _ => unreachable!(),
        }
    }

    assert_eq!(keyed_array[1].to_str().unwrap(), "bar");

    for (i, elem) in std::mem::take(keyed_array).into_iter().enumerate() {
        match i {
            0 => assert_eq!(elem.to_u64().unwrap(), 1),
            1 => assert_eq!(elem.to_str().unwrap(), "bar"),
            _ => unreachable!(),
        }
    }
}

#[test]
fn iteration_of_keyed_map() {
    let mut map = waf_map!(("key1", waf_map!(("key2", 1u64))));
    let keyed_map: &mut Keyed<WafMap> = map[0].as_type_mut().unwrap();

    for (i, elem) in keyed_map.iter().enumerate() {
        match i {
            0 => {
                assert_eq!(elem.key_str().unwrap(), "key2");
                assert_eq!(elem.to_u64().unwrap(), 1);
            }
            _ => unreachable!(),
        }
    }

    for (i, elem) in keyed_map.iter_mut().enumerate() {
        match i {
            0 => {
                assert_eq!(elem.key_str().unwrap(), "key2");
                assert_eq!(elem.to_u64().unwrap(), 1);
                let new_val: Keyed<WafString> = ("new_key", "bar").into();
                let _ = std::mem::replace(elem, new_val.into());
            }
            _ => unreachable!(),
        }
    }
    assert_eq!(keyed_map[0].key_str().unwrap(), "new_key");
    assert_eq!(keyed_map[0].to_str().unwrap(), "bar");

    for (i, elem) in std::mem::take(keyed_map).into_iter().enumerate() {
        match i {
            0 => {
                assert_eq!(elem.key_str().unwrap(), "new_key");
                assert_eq!(elem.to_str().unwrap(), "bar");
            }
            _ => unreachable!(),
        }
    }
}

#[test]
#[allow(clippy::float_cmp)] // No operations performed on the floats, they should be identical.
fn from_implementations() {
    assert_eq!(WafSigned::from(-123i64).value(), -123);
    assert_eq!(WafSigned::from(-123i32).value(), -123);

    assert_eq!(WafUnsigned::from(123u64).value(), 123);
    assert_eq!(WafUnsigned::from(123u32).value(), 123);

    assert_eq!(
        WafString::from("Hello, world!").as_str(),
        Ok("Hello, world!")
    );
    assert_eq!(
        WafString::from(b"Hello, world!").as_str(),
        Ok("Hello, world!")
    );

    let arr = WafArray::from([1u64, 2u64, 3u64]);
    for (i, elem) in arr.iter().enumerate() {
        assert_eq!(elem.to_u64().unwrap(), i as u64 + 1);
    }

    let map = WafMap::from([("1", 1u64), ("2", 2u64)]);
    for elem in map {
        let key = elem.key_str().unwrap();
        let val = elem.to_u64().unwrap();
        assert_eq!(key, format!("{val}"));
    }

    assert!(WafBool::from(true).value());
    assert!(!WafBool::from(false).value());

    assert_eq!(WafFloat::from(1.0).value(), 1.0);

    assert!(WafNull::from(()).is_valid());
}

#[test]
fn try_from_implementations() {
    assert!(matches!(
        WafSigned::try_from(WafObject::default()),
        Err(ObjectTypeError {
            expected: WafObjectType::Signed,
            actual: WafObjectType::Invalid
        })
    ));
    assert!(matches!(
        WafUnsigned::try_from(WafObject::default()),
        Err(ObjectTypeError {
            expected: WafObjectType::Unsigned,
            actual: WafObjectType::Invalid
        })
    ));
    assert!(matches!(
        WafString::try_from(WafObject::default()),
        Err(ObjectTypeError {
            expected: WafObjectType::String,
            actual: WafObjectType::Invalid
        })
    ));
    assert!(matches!(
        WafArray::try_from(WafObject::default()),
        Err(ObjectTypeError {
            expected: WafObjectType::Array,
            actual: WafObjectType::Invalid
        })
    ));
    assert!(matches!(
        WafMap::try_from(WafObject::default()),
        Err(ObjectTypeError {
            expected: WafObjectType::Map,
            actual: WafObjectType::Invalid
        })
    ));
    assert!(matches!(
        WafBool::try_from(WafObject::default()),
        Err(ObjectTypeError {
            expected: WafObjectType::Bool,
            actual: WafObjectType::Invalid
        })
    ));
    assert!(matches!(
        WafFloat::try_from(WafObject::default()),
        Err(ObjectTypeError {
            expected: WafObjectType::Float,
            actual: WafObjectType::Invalid
        })
    ));
    assert!(matches!(
        WafNull::try_from(WafObject::default()),
        Err(ObjectTypeError {
            expected: WafObjectType::Null,
            actual: WafObjectType::Invalid
        })
    ));

    let obj = waf_object!(42u64);
    assert!(WafArray::try_from(obj).is_err());
    let obj = waf_object!(42u64);
    assert!(WafUnsigned::try_from(obj).is_ok());

    let obj = waf_object!(42);
    assert!(WafUnsigned::try_from(obj).is_err());
    let obj = waf_object!(42);
    assert!(WafSigned::try_from(obj).is_ok());

    let obj = waf_object!(42.0);
    assert!(WafSigned::try_from(obj).is_err());
    let obj = waf_object!(42.0);
    assert!(WafFloat::try_from(obj).is_ok());

    let obj = waf_object!(true);
    assert!(WafFloat::try_from(obj).is_err());
    let obj = waf_object!(true);
    assert!(WafBool::try_from(obj).is_ok());

    let obj = waf_object!(null);
    assert!(WafBool::try_from(obj).is_err());
    let obj = waf_object!(null);
    assert!(WafNull::try_from(obj).is_ok());

    let obj = waf_object!("foobar");
    assert!(WafNull::try_from(obj).is_err());
    let obj = waf_object!("foobar");
    assert!(WafString::try_from(obj).is_ok());

    let obj: WafObject = waf_map!().into();
    assert!(WafString::try_from(obj).is_err());
    let obj: WafObject = waf_map!().into();
    assert!(WafMap::try_from(obj).is_ok());

    let obj: WafObject = waf_array!().into();
    assert!(WafMap::try_from(obj).is_err());
    let obj: WafObject = waf_array!().into();
    assert!(WafArray::try_from(obj).is_ok());
}

#[test]
#[cfg(not(miri))]
fn test_from_json() {
    let json = r#"{
                "null": null,
                "unsigned": 1,
                "signed": -1,
                "string": "foo",
                "bool.true": true,
                "bool.false": false,
                "array": [false, 1, "two", null]
            }"#;
    let expected = WafObject::from(waf_map! {
        ("null", WafNull::default()),
        ("unsigned", WafUnsigned::from(1u64)),
        ("signed", WafSigned::from(-1i64)),
        ("string", WafString::from("foo")),
        ("bool.true", WafBool::from(true)),
        ("bool.false", WafBool::from(false)),
        ("array", WafArray::from([WafObject::from(false), WafObject::from(1u64), WafObject::from("two"), WafNull::default().into()])),
    });

    assert_eq!(
        WafObject::from_json(json).expect("should have succeeded"),
        expected
    );
    assert_eq!(
        WafOwnedDefaultAllocator::<WafObject>::from_json(json).expect("should have succeeded"),
        expected
    );
    assert_eq!(
        WafOwnedOutputAllocator::<WafObject>::from_json(json).expect("should have succeeded"),
        expected
    );

    // No data
    assert!(WafObject::from_json("").is_none());
    assert!(WafOwnedDefaultAllocator::<WafObject>::from_json("").is_none());
    // Invalid JSON (truncated)
    assert!(WafObject::from_json("{").is_none());
    assert!(WafOwnedDefaultAllocator::<WafObject>::from_json("{").is_none());
    // Too large (but otherwise valid)
    assert!(WafObject::from_json(format!(r#""{}""#, "a".repeat(u32::MAX as usize + 1))).is_none());
    assert!(WafOwnedDefaultAllocator::<WafObject>::from_json(format!(
        r#""{}""#,
        "a".repeat(u32::MAX as usize + 1)
    ))
    .is_none());
}

#[test]
#[cfg(not(miri))] // takes too long
fn test_array_from_large_slice_truncates() {
    const EXCESS_SIZE: usize = u16::MAX as usize + 1;
    let mut large_vec: Vec<WafObject> = (0..EXCESS_SIZE)
        .map(|i| WafObject::from(i as u64))
        .collect();

    let array = WafArray::from(&mut large_vec[..]);

    // Verify the array was truncated to u16::MAX
    assert_eq!(array.len(), u16::MAX);
    assert_eq!(array.capacity(), u16::MAX);

    // Verify the elements in the array are correct
    assert_eq!(array[0].to_u64().unwrap(), 0);
    assert_eq!(array[100].to_u64().unwrap(), 100);
    assert_eq!(array[65534].to_u64().unwrap(), 65534);

    // After From<&mut [T]>, the first u16::MAX elements in the vec are taken (replaced with default)
    // The element at index 65535 (the excess one) should still exist
    assert_eq!(large_vec.len(), EXCESS_SIZE);
    assert_eq!(large_vec[0].object_type(), WafObjectType::Invalid);
    assert_eq!(large_vec[65535].to_u64().unwrap(), 65535);
}

#[test]
#[cfg(not(miri))] // takes too long
fn test_array_from_large_array_truncates() {
    const EXCESS_SIZE: usize = u16::MAX as usize + 1;
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024) // 32 MB stack
        .spawn(|| {
            let large_array: [WafObject; EXCESS_SIZE] = std::array::from_fn(|i| (i as u64).into());
            let array = WafArray::from(large_array);

            assert_eq!(array.len(), u16::MAX);
            assert_eq!(array.capacity(), u16::MAX);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[cfg(not(miri))] // takes too long
fn test_map_from_large_slice_truncates() {
    const EXCESS_SIZE: usize = u16::MAX as usize + 1;
    let mut large_vec: Vec<(WafObject, WafObject)> = (0..EXCESS_SIZE)
        .map(|i| {
            (
                WafObject::from(format!("key{}", i).as_str()),
                WafObject::from(i as u64),
            )
        })
        .collect();

    let map = WafMap::from(&mut large_vec[..]);

    assert_eq!(map.len(), u16::MAX);
    assert_eq!(map.capacity(), u16::MAX);

    assert_eq!(map.get_str("key0").unwrap().to_u64().unwrap(), 0);
    assert_eq!(map.get_str("key100").unwrap().to_u64().unwrap(), 100);
    assert_eq!(map.get_str("key65534").unwrap().to_u64().unwrap(), 65534);

    assert_eq!(large_vec.len(), EXCESS_SIZE);
    assert_eq!(large_vec[0].0.object_type(), WafObjectType::Invalid);
    assert_eq!(large_vec[0].1.object_type(), WafObjectType::Invalid);
    assert_eq!(large_vec[65535].0.to_str().unwrap(), "key65535");
    assert_eq!(large_vec[65535].1.to_u64().unwrap(), 65535);
}

#[test]
#[cfg(not(miri))] // takes too long
fn test_map_from_large_array_truncates() {
    const EXCESS_SIZE: usize = u16::MAX as usize + 1;
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024) // 32 MB stack
        .spawn(|| {
            let large_array: [(WafObject, WafObject); EXCESS_SIZE] = std::array::from_fn(|i| {
                (
                    WafObject::from(format!("key{}", i).as_str()),
                    WafObject::from(i as u64),
                )
            });
            let map = WafMap::from(large_array);
            assert_eq!(map.len(), u16::MAX);
            assert_eq!(map.capacity(), u16::MAX);
        })
        .unwrap()
        .join()
        .unwrap();
}
