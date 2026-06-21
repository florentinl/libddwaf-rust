#![doc = "Data model for exchanging data with the in-app WAF."]

use std::alloc::Layout;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::ptr::null_mut;
use std::sync::OnceLock;
use std::{cmp, fmt};

mod iter;
#[doc(inline)]
pub use iter::*;

/// Identifies the type of the value stored in a [`WafObject`].
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WafObjectType {
    /// An invalid value. This can be used as a placeholder to retain the key
    /// associated with an object that was only partially encoded.
    Invalid,
    /// A signed integer with 64-bit precision.
    Signed,
    /// An unsigned integer with 64-bit precision.
    Unsigned,
    /// A string value.
    String,
    /// An array of [`WafObject`]s.
    Array,
    /// An array of [`Keyed<WafObject>`]s.
    Map,
    /// A boolean value.
    Bool,
    /// A floating point value (64-bit precision).
    Float,
    /// The null value.
    Null,
}
impl WafObjectType {
    /// Returns the raw [`libddwaf_sys::DDWAF_OBJ_TYPE`] value corresponding to this [`WafObjectType`].
    const fn as_raw(self) -> libddwaf_sys::DDWAF_OBJ_TYPE {
        match self {
            WafObjectType::Invalid => libddwaf_sys::DDWAF_OBJ_INVALID,
            WafObjectType::Signed => libddwaf_sys::DDWAF_OBJ_SIGNED,
            WafObjectType::Unsigned => libddwaf_sys::DDWAF_OBJ_UNSIGNED,
            WafObjectType::String => libddwaf_sys::DDWAF_OBJ_STRING,
            WafObjectType::Array => libddwaf_sys::DDWAF_OBJ_ARRAY,
            WafObjectType::Map => libddwaf_sys::DDWAF_OBJ_MAP,
            WafObjectType::Bool => libddwaf_sys::DDWAF_OBJ_BOOL,
            WafObjectType::Float => libddwaf_sys::DDWAF_OBJ_FLOAT,
            WafObjectType::Null => libddwaf_sys::DDWAF_OBJ_NULL,
        }
    }
}
impl TryFrom<libddwaf_sys::DDWAF_OBJ_TYPE> for WafObjectType {
    type Error = UnknownObjectTypeError;
    fn try_from(value: libddwaf_sys::DDWAF_OBJ_TYPE) -> Result<Self, UnknownObjectTypeError> {
        match value {
            libddwaf_sys::DDWAF_OBJ_INVALID => Ok(WafObjectType::Invalid),
            libddwaf_sys::DDWAF_OBJ_SIGNED => Ok(WafObjectType::Signed),
            libddwaf_sys::DDWAF_OBJ_UNSIGNED => Ok(WafObjectType::Unsigned),
            libddwaf_sys::DDWAF_OBJ_STRING
            | libddwaf_sys::DDWAF_OBJ_LITERAL_STRING
            | libddwaf_sys::DDWAF_OBJ_SMALL_STRING => Ok(WafObjectType::String),
            libddwaf_sys::DDWAF_OBJ_ARRAY => Ok(WafObjectType::Array),
            libddwaf_sys::DDWAF_OBJ_MAP => Ok(WafObjectType::Map),
            libddwaf_sys::DDWAF_OBJ_BOOL => Ok(WafObjectType::Bool),
            libddwaf_sys::DDWAF_OBJ_FLOAT => Ok(WafObjectType::Float),
            libddwaf_sys::DDWAF_OBJ_NULL => Ok(WafObjectType::Null),
            unknown => Err(UnknownObjectTypeError(unknown)),
        }
    }
}

/// The error that is returned when a [`WafObject`] does not have a known, valid [`WafObjectType`].
#[derive(Copy, Clone, Debug)]
pub struct UnknownObjectTypeError(libddwaf_sys::DDWAF_OBJ_TYPE);
impl std::error::Error for UnknownObjectTypeError {}
impl std::fmt::Display for UnknownObjectTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unknown object type: {:?}", self.0)
    }
}

/// The error that is returned when a [`WafObject`] does not have the expected [`WafObjectType`].
#[derive(Copy, Clone, Debug)]
pub struct ObjectTypeError {
    pub expected: WafObjectType,
    pub actual: WafObjectType,
}
impl std::error::Error for ObjectTypeError {}
impl std::fmt::Display for ObjectTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid object type (expected {:?}, got {:?})",
            self.expected, self.actual
        )
    }
}

/// The error that is returned when a value's length exceeds the maximum allowed.
///
/// This applies to strings (max [`u32::MAX`]) and arrays/maps (max [`u16::MAX`]).
#[derive(Copy, Clone, Debug)]
pub struct LengthTooLargeError {
    /// The length that was too large.
    pub length: usize,
    /// The maximum allowed length.
    pub max_length: usize,
}
impl std::error::Error for LengthTooLargeError {}
impl std::fmt::Display for LengthTooLargeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Length {} exceeds maximum allowed {}",
            self.length, self.max_length
        )
    }
}

/// This trait allow obtaining direct mutable access to the underlying memory
/// backing a [`WafObject`] or [`TypedWafObject`] value.
#[doc(hidden)]
pub trait AsRawMutObject: crate::private::Sealed + AsRef<libddwaf_sys::ddwaf_object> {
    /// Obtains a mutable reference to the underlying raw [`libddwaf_sys::ddwaf_object`].
    ///
    /// # Safety
    /// The caller must ensure that:
    /// - it does not change the [`libddwaf_sys::ddwaf_object::type_`] field,
    /// - it does not change the pointers to values that don't outlive the [`libddwaf_sys::ddwaf_object`]
    ///   itself, or whose memory cannot be recclaimed byt the destructor in the same way as the
    ///   current value,
    /// - it does not change the lengths in such a way that the object is no longer valid.
    ///
    /// Additionally, the caller would incur a memory leak if it dropped the value through the
    /// returned reference (e.g, by calling [`std::mem::replace`]), since [`libddwaf_sys::ddwaf_object`] is
    /// not [`Drop`] (see swapped destructors in
    /// [this playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=aeea4aba8f960bf0c63f6185f016a94d).
    #[doc(hidden)]
    unsafe fn as_raw_mut(&mut self) -> &mut libddwaf_sys::ddwaf_object;
}

/// This trait is implemented by type-safe interfaces to the [`WafObject`], with
/// one implementation for each [`WafObjectType`].
pub trait TypedWafObject: AsRawMutObject {
    /// The associated [`WafObjectType`] constant corresponding to the typed
    /// object's type discriminator.
    const TYPE: WafObjectType;
}

/// The low-level representation of an arbitrary WAF object.
///
/// It is usually converted to a [`TypedWafObject`] by calling [`WafObject::as_type`].
#[derive(Default)]
#[repr(transparent)]
pub struct WafObject {
    raw: libddwaf_sys::ddwaf_object,
}
impl WafObject {
    /// Creates a new [`WafObject`] from a JSON string.
    ///
    /// This function is not intended to be used with un-trusted/adversarial
    /// input. The typical use-case is to facilitate parsing rulesets for use
    /// with [`crate::builder::Builder::add_or_update_config`].
    ///
    /// # Returns
    /// Returns [`None`] if parsing the JSON string into a [`WafObject`] was not
    /// possible, or if the input JSON string is larger than [`u32::MAX`] bytes.
    pub fn from_json(json: impl AsRef<[u8]>) -> Option<WafOwnedOutputAllocator<Self>> {
        WafOwnedOutputAllocator::<Self>::from_json(json)
    }

    /// Returns the [`WafObjectType`] of the underlying value.
    ///
    /// Returns [`WafObjectType::Invalid`] if the underlying value's type is not set to a
    /// known, valid [`WafObjectType`] value.
    #[must_use]
    pub fn object_type(&self) -> WafObjectType {
        self.as_ref()
            .obj_type()
            .try_into()
            .unwrap_or(WafObjectType::Invalid)
    }

    /// Returns a reference to this value as a `T` if its type corresponds.
    #[must_use]
    pub fn as_type<T: TypedWafObject>(&self) -> Option<&T> {
        if self.object_type() == T::TYPE {
            Some(unsafe { self.as_type_unchecked::<T>() })
        } else {
            None
        }
    }

    /// Returns a reference to this value as a `T`.
    ///
    /// # Safety
    /// The caller must ensure that the [`WafObject`] can be accurately represented by `T`.
    pub(crate) unsafe fn as_type_unchecked<T: TypedWafObject>(&self) -> &T {
        unsafe { self.as_ref().unchecked_as_ref::<T>() }
    }

    /// Returns a mutable reference to this value as a `T` if its type corresponds.
    pub fn as_type_mut<T: TypedWafObject>(&mut self) -> Option<&mut T> {
        if self.object_type() == T::TYPE {
            Some(unsafe { self.as_raw_mut().unchecked_as_ref_mut::<T>() })
        } else {
            None
        }
    }

    /// Returns true if this [`WafObject`] is not [`WafObjectType::Invalid`], meaning it can be
    /// converted to one of the [`TypedWafObject`] implementations.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.object_type() != WafObjectType::Invalid
    }

    /// Returns the value of this [`WafObject`] as a [`u64`] if its type is [`WafObjectType::Unsigned`].
    #[must_use]
    pub fn to_u64(&self) -> Option<u64> {
        self.as_type::<WafUnsigned>().map(WafUnsigned::value)
    }

    /// Returns the value of this [`WafObject`] as a [`i64`] if its type is [`WafObjectType::Signed`] (or
    /// [`WafObjectType::Unsigned`] with a value that can be represented as an [`i64`]).
    #[must_use]
    pub fn to_i64(&self) -> Option<i64> {
        match self.object_type() {
            WafObjectType::Unsigned => {
                let obj: &WafUnsigned = unsafe { self.as_type_unchecked() };
                obj.value().try_into().ok()
            }
            WafObjectType::Signed => {
                let obj: &WafSigned = unsafe { self.as_type_unchecked() };
                Some(obj.value())
            }
            _ => None,
        }
    }

    /// Returns the value of this [`WafObject`] as a [`f64`] if its type is [`WafObjectType::Float`].
    #[must_use]
    pub fn to_f64(&self) -> Option<f64> {
        self.as_type::<WafFloat>().map(WafFloat::value)
    }

    /// Returns the value of this [`WafObject`] as a [`bool`] if its type is [`WafObjectType::Bool`].
    #[must_use]
    pub fn to_bool(&self) -> Option<bool> {
        self.as_type::<WafBool>().map(WafBool::value)
    }

    /// Returns the value of this [`WafObject`] as a [`&str`] if its type is [`WafObjectType::String`],
    /// and the value is valid UTF-8.
    #[must_use]
    pub fn to_str(&self) -> Option<&str> {
        self.as_type::<WafString>().and_then(|x| x.as_str().ok())
    }

    /// Initializes this object as a null value.
    ///
    /// This is intended for fresh or invalid objects, such as slots returned by
    /// [`WafObject::insert`] or [`WafObject::insert_key`].
    pub fn set_null(&mut self) -> Option<&mut Self> {
        let ptr = unsafe { libddwaf_sys::ddwaf_object_set_null(self.as_raw_mut()) };
        (!ptr.is_null()).then_some(self)
    }

    /// Initializes this object as a signed integer.
    ///
    /// This is intended for fresh or invalid objects, such as slots returned by
    /// [`WafObject::insert`] or [`WafObject::insert_key`].
    pub fn set_signed(&mut self, value: i64) -> Option<&mut Self> {
        let ptr = unsafe { libddwaf_sys::ddwaf_object_set_signed(self.as_raw_mut(), value) };
        (!ptr.is_null()).then_some(self)
    }

    /// Initializes this object as a boolean.
    ///
    /// This is intended for fresh or invalid objects, such as slots returned by
    /// [`WafObject::insert`] or [`WafObject::insert_key`].
    pub fn set_bool(&mut self, value: bool) -> Option<&mut Self> {
        let ptr = unsafe { libddwaf_sys::ddwaf_object_set_bool(self.as_raw_mut(), value) };
        (!ptr.is_null()).then_some(self)
    }

    /// Initializes this object as a floating point value.
    ///
    /// This is intended for fresh or invalid objects, such as slots returned by
    /// [`WafObject::insert`] or [`WafObject::insert_key`].
    pub fn set_float(&mut self, value: f64) -> Option<&mut Self> {
        let ptr = unsafe { libddwaf_sys::ddwaf_object_set_float(self.as_raw_mut(), value) };
        (!ptr.is_null()).then_some(self)
    }

    /// Initializes this object as a copied string using allocator `A`.
    ///
    /// This is intended for fresh or invalid objects, such as slots returned by
    /// [`WafObject::insert`] or [`WafObject::insert_key`]. The string bytes are
    /// copied by libddwaf, so the input slice only needs to live for this call.
    pub fn set_string<A: AllocatorType>(&mut self, value: impl AsRef<[u8]>) -> Option<&mut Self> {
        let value = value.as_ref();
        let len = u32::try_from(value.len()).ok()?;
        let ptr = unsafe {
            libddwaf_sys::ddwaf_object_set_string(
                self.as_raw_mut(),
                value.as_ptr().cast(),
                len,
                A::allocator(),
            )
        };
        (!ptr.is_null()).then_some(self)
    }

    /// Initializes this object as an array with the provided capacity using allocator `A`.
    ///
    /// This is intended for fresh or invalid objects.
    pub fn set_array<A: AllocatorType>(&mut self, capacity: u16) -> Option<&mut Self> {
        let ptr = unsafe {
            libddwaf_sys::ddwaf_object_set_array(self.as_raw_mut(), capacity, A::allocator())
        };
        (!ptr.is_null()).then_some(self)
    }

    /// Initializes this object as a map with the provided capacity using allocator `A`.
    ///
    /// This is intended for fresh or invalid objects.
    pub fn set_map<A: AllocatorType>(&mut self, capacity: u16) -> Option<&mut Self> {
        let ptr = unsafe {
            libddwaf_sys::ddwaf_object_set_map(self.as_raw_mut(), capacity, A::allocator())
        };
        (!ptr.is_null()).then_some(self)
    }

    /// Inserts a new value slot into this array using allocator `A`.
    ///
    /// Returns [`None`] if this object is not an array, the array is full, or allocation fails.
    pub fn insert<A: AllocatorType>(&mut self) -> Option<&mut WafObject> {
        let ptr = unsafe { libddwaf_sys::ddwaf_object_insert(self.as_raw_mut(), A::allocator()) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &mut *ptr.cast::<WafObject>() })
        }
    }

    /// Inserts a new value slot into this map with a copied key using allocator `A`.
    ///
    /// Returns [`None`] if this object is not a map, the map is full, the key is too large, or
    /// allocation fails. The key bytes are copied by libddwaf, so the input slice only needs to live
    /// for this call.
    pub fn insert_key<A: AllocatorType>(
        &mut self,
        key: impl AsRef<[u8]>,
    ) -> Option<&mut WafObject> {
        let key = key.as_ref();
        let len = u32::try_from(key.len()).ok()?;
        let ptr = unsafe {
            libddwaf_sys::ddwaf_object_insert_key(
                self.as_raw_mut(),
                key.as_ptr().cast(),
                len,
                A::allocator(),
            )
        };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &mut *ptr.cast::<WafObject>() })
        }
    }
}
impl AsRef<libddwaf_sys::ddwaf_object> for WafObject {
    fn as_ref(&self) -> &libddwaf_sys::ddwaf_object {
        &self.raw
    }
}
impl AsRawMutObject for WafObject {
    unsafe fn as_raw_mut(&mut self) -> &mut libddwaf_sys::ddwaf_object {
        &mut self.raw
    }
}
impl fmt::Debug for WafObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.object_type() {
            WafObjectType::Invalid => write!(f, "WafInvalid"),
            WafObjectType::Unsigned => {
                let obj: &WafUnsigned = self.as_type().unwrap();
                obj.fmt(f)
            }
            WafObjectType::Signed => {
                let obj: &WafSigned = self.as_type().unwrap();
                obj.fmt(f)
            }
            WafObjectType::Float => {
                let obj: &WafFloat = self.as_type().unwrap();
                obj.fmt(f)
            }
            WafObjectType::Bool => {
                let obj: &WafBool = self.as_type().unwrap();
                obj.fmt(f)
            }
            WafObjectType::Null => {
                let obj: &WafNull = self.as_type().unwrap();
                obj.fmt(f)
            }
            WafObjectType::String => {
                let obj: &WafString = self.as_type().unwrap();
                obj.fmt(f)
            }
            WafObjectType::Array => {
                let obj: &WafArray = self.as_type().unwrap();
                obj.fmt(f)
            }
            WafObjectType::Map => {
                let obj: &WafMap = self.as_type().unwrap();
                obj.fmt(f)
            }
        }
    }
}
impl Drop for WafObject {
    fn drop(&mut self) {
        unsafe { self.raw.drop_object() }
    }
}
impl Clone for WafObject {
    fn clone(&self) -> Self {
        match self.object_type() {
            WafObjectType::Invalid => {
                let obj: &WafInvalid = unsafe { self.as_type_unchecked() };
                (*obj).into()
            }
            WafObjectType::Signed => {
                let obj: &WafSigned = unsafe { self.as_type_unchecked() };
                (*obj).into()
            }
            WafObjectType::Unsigned => {
                let obj: &WafUnsigned = unsafe { self.as_type_unchecked() };
                (*obj).into()
            }
            WafObjectType::Bool => {
                let obj: &WafBool = unsafe { self.as_type_unchecked() };
                (*obj).into()
            }
            WafObjectType::Float => {
                let obj: &WafFloat = unsafe { self.as_type_unchecked() };
                (*obj).into()
            }
            WafObjectType::Null => {
                let obj: &WafNull = unsafe { self.as_type_unchecked() };
                (*obj).into()
            }
            WafObjectType::String => {
                let obj: &WafString = unsafe { self.as_type_unchecked() };
                obj.clone().into()
            }
            WafObjectType::Array => {
                let obj: &WafArray = unsafe { self.as_type_unchecked() };
                obj.clone().into()
            }
            WafObjectType::Map => {
                let obj: &WafMap = unsafe { self.as_type_unchecked() };
                obj.clone().into()
            }
        }
    }
}
impl From<u64> for WafObject {
    fn from(value: u64) -> Self {
        WafUnsigned::new(value).into()
    }
}
impl From<u32> for WafObject {
    fn from(value: u32) -> Self {
        WafUnsigned::new(value.into()).into()
    }
}
impl From<i64> for WafObject {
    fn from(value: i64) -> Self {
        WafSigned::new(value).into()
    }
}
impl From<i32> for WafObject {
    fn from(value: i32) -> Self {
        WafSigned::new(value.into()).into()
    }
}
impl From<f64> for WafObject {
    fn from(value: f64) -> Self {
        WafFloat::new(value).into()
    }
}
impl From<bool> for WafObject {
    fn from(value: bool) -> Self {
        WafBool::new(value).into()
    }
}
impl From<&str> for WafObject {
    fn from(value: &str) -> Self {
        value.as_bytes().into()
    }
}
impl From<&[u8]> for WafObject {
    fn from(value: &[u8]) -> Self {
        WafString::from(value).into()
    }
}
impl From<()> for WafObject {
    fn from((): ()) -> Self {
        WafNull::new().into()
    }
}
impl<T: TypedWafObject> From<T> for WafObject {
    fn from(value: T) -> Self {
        let res = Self {
            raw: *value.as_ref(),
        };
        std::mem::forget(value);
        res
    }
}
impl<T: AsRef<libddwaf_sys::ddwaf_object>> cmp::PartialEq<T> for WafObject {
    fn eq(&self, other: &T) -> bool {
        self.raw == *other.as_ref()
    }
}
impl crate::private::Sealed for WafObject {}

/// Trait to encode which allocator should be used for deallocation in the type system.
pub trait AllocatorType: 'static {
    /// Get the allocator to use for deallocation.
    fn allocator() -> libddwaf_sys::ddwaf_allocator;
}

/// Allocator type that uses libddwaf's default.
pub struct LibddwafDefaultAllocator;
impl AllocatorType for LibddwafDefaultAllocator {
    fn allocator() -> libddwaf_sys::ddwaf_allocator {
        unsafe { libddwaf_sys::ddwaf_get_default_allocator() }
    }
}

/// Allocator type that uses the Rust-registered allocator.
pub struct RustAllocator;
impl AllocatorType for RustAllocator {
    fn allocator() -> libddwaf_sys::ddwaf_allocator {
        get_default_allocator().into()
    }
}

/// A WAF-owned [`WafObject`] or [`TypedWafObject`] value.
///
/// This has different [`Drop`] behavior than a rust-owned [`WafObject`] value.
/// The allocator used for deallocation is encoded in the type parameter `A`.
#[repr(transparent)]
pub struct WafOwned<T: AsRawMutObject, A: AllocatorType = RustAllocator> {
    inner: std::mem::ManuallyDrop<T>,
    _phantom: std::marker::PhantomData<A>,
}
impl<T: AsRawMutObject, A: AllocatorType> WafOwned<T, A> {
    pub(crate) fn allocator() -> libddwaf_sys::ddwaf_allocator {
        A::allocator()
    }
}

impl<A: AllocatorType> WafOwned<WafObject, A> {
    /// Creates a new owned [`WafObject`] from a JSON string using this owned object's allocator.
    ///
    /// This is useful when the parsed object should be freed with a specific libddwaf allocator,
    /// such as [`WafOwnedDefaultAllocator`] for input/config objects.
    ///
    /// # Returns
    /// Returns [`None`] if parsing the JSON string into a [`WafObject`] was not
    /// possible, or if the input JSON string is larger than [`u32::MAX`] bytes.
    pub fn from_json(json: impl AsRef<[u8]>) -> Option<Self> {
        let mut output = Self::default();
        let data = json.as_ref();
        let Ok(len) = u32::try_from(data.len()) else {
            return None;
        };
        if !unsafe {
            libddwaf_sys::ddwaf_object_from_json(
                output.as_raw_mut(),
                data.as_ptr().cast(),
                len,
                Self::allocator(),
            )
        } {
            return None;
        }
        Some(output)
    }
}

impl<T: AsRawMutObject + fmt::Debug, A: AllocatorType> fmt::Debug for WafOwned<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.deref().fmt(f)
    }
}
impl<T: AsRawMutObject + Default, A: AllocatorType> Default for WafOwned<T, A> {
    fn default() -> Self {
        Self {
            inner: std::mem::ManuallyDrop::new(Default::default()),
            _phantom: std::marker::PhantomData,
        }
    }
}
impl<T: AsRawMutObject, A: AllocatorType> Deref for WafOwned<T, A> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T: AsRawMutObject, A: AllocatorType> DerefMut for WafOwned<T, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl<T: AsRawMutObject, A: AllocatorType> AsRef<libddwaf_sys::ddwaf_object> for WafOwned<T, A> {
    fn as_ref(&self) -> &libddwaf_sys::ddwaf_object {
        self.inner.as_ref()
    }
}
impl<T: AsRawMutObject, A: AllocatorType> AsRawMutObject for WafOwned<T, A> {
    unsafe fn as_raw_mut(&mut self) -> &mut libddwaf_sys::ddwaf_object {
        unsafe { self.inner.as_raw_mut() }
    }
}
impl<T: AsRawMutObject, A: AllocatorType> Drop for WafOwned<T, A> {
    fn drop(&mut self) {
        unsafe {
            libddwaf_sys::ddwaf_object_destroy(self.inner.as_raw_mut(), A::allocator());
        }
    }
}
impl<T: AsRawMutObject, A: AllocatorType> PartialEq<T> for WafOwned<T, A>
where
    T: PartialEq<T>,
{
    fn eq(&self, other: &T) -> bool {
        *self.inner == *other
    }
}
impl<T: AsRawMutObject, A: AllocatorType> crate::private::Sealed for WafOwned<T, A> {}

/// Type alias for WAF-owned objects using the system default allocator.
pub type WafOwnedDefaultAllocator<T> = WafOwned<T, LibddwafDefaultAllocator>;

/// Type alias for WAF-owned objects using the Rust-registered allocator (for outputs).
pub type WafOwnedOutputAllocator<T> = WafOwned<T, RustAllocator>;

/// Allocates memory for the given [`Layout`], calling [`std::alloc::handle_alloc_error`] if the
/// allocation failed.
///
/// # Safety
/// The requirements as for [`std::alloc::alloc`] apply.
unsafe fn no_fail_alloc(layout: Layout) -> *mut u8 {
    if layout.size() == 0 {
        return null_mut();
    }
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    ptr
}

macro_rules! typed_object {
    (@defaults $type:expr, $name:ident) => {
        #[doc = concat!("Returns true if this [", stringify!($name), "] is indeed [", stringify!($type), "].")]
        #[must_use]
        pub fn is_valid(&self) -> bool {
            self.raw.obj_type() == $type.as_raw()
        }
    };
    (@defaults $type:expr, $name:ident, $($is_valid:tt)*) => {
        #[doc = concat!("Returns true if this [", stringify!($name), "] is indeed [", stringify!($type), "].")]
        #[must_use]
        $($is_valid)*
    };
    ($type:expr => $name:ident $(derive($($derives:ident),* $(,)?))? $(is_valid { $($is_valid:tt)* })? $({ $($impl:tt)* })?) => {
        #[doc = concat!("The WAF object representation of a value of type [", stringify!($type), "]")]
        #[repr(transparent)]
        $(#[derive($($derives),*)] )?
        pub struct $name {
            raw: libddwaf_sys::ddwaf_object,
        }
        impl $name {
            typed_object!(@defaults $type, $name $(, $($is_valid)*)?);

            /// Returns a reference to this value as a [`WafObject`].
            #[must_use]
            pub fn as_object(&self) -> &WafObject{
                let obj: &libddwaf_sys::ddwaf_object = self.as_ref();
                obj.as_object_ref()
            }
            $(
            $($impl)*)?
        }
        impl AsRef<libddwaf_sys::ddwaf_object> for $name {
            fn as_ref(&self) -> &libddwaf_sys::ddwaf_object {
                &self.raw
            }
        }
        impl AsRawMutObject for $name {
            unsafe fn as_raw_mut(&mut self) -> &mut libddwaf_sys::ddwaf_object {
                &mut self.raw
            }
        }
        impl Default for $name {
            #[allow(clippy::cast_possible_truncation)]
            fn default() -> Self {
                // All the types admit this representation
                let mut raw: libddwaf_sys::ddwaf_object = unsafe { std::mem::zeroed() };
                raw.type_ = $type.as_raw() as u8;
                Self { raw }
            }
        }
        impl TryFrom<WafObject> for $name {
            type Error = ObjectTypeError;
            fn try_from(obj: WafObject) -> Result<Self, Self::Error> {
                if obj.object_type() != Self::TYPE {
                    return Err(ObjectTypeError {
                        expected: $type,
                        actual: obj.object_type(),
                    });
                }
                let res = Self { raw: obj.raw };
                std::mem::forget(obj);
                Ok(res)
            }
        }
        impl<T: AsRef<libddwaf_sys::ddwaf_object>> cmp::PartialEq<T> for $name {
            fn eq(&self, other: &T) -> bool {
                self.raw == *other.as_ref()
            }
        }
        impl crate::private::Sealed for $name {}
        impl TypedWafObject for $name {
            const TYPE: WafObjectType = $type;
        }
    };
}

typed_object!(WafObjectType::Invalid => WafInvalid derive(Copy, Clone));

typed_object!(WafObjectType::Signed => WafSigned derive(Copy, Clone) {
    /// Creates a new [`WafSigned`] with the provided value.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn new(val: i64) -> Self {
        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    i64_: libddwaf_sys::_ddwaf_object_signed {
                        type_: libddwaf_sys::DDWAF_OBJ_SIGNED as u8,
                        val,
                    },
                },
            }
        }
    }

    /// Returns the value of this [`WafSigned`].
    #[must_use]
    pub const fn value(&self) -> i64 {
        unsafe { self.raw.via.i64_.val }
    }
});

typed_object!(WafObjectType::Unsigned => WafUnsigned derive(Copy, Clone) {
    /// Creates a new [`WafUnsigned`] with the provided value.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn new(val: u64) -> Self {
        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    u64_: libddwaf_sys::_ddwaf_object_unsigned {
                        type_: libddwaf_sys::DDWAF_OBJ_UNSIGNED as u8,
                        val,
                    },
                },
            }
        }
    }

    /// Returns the value of this [`WafUnsigned`].
    #[must_use]
    pub const fn value(&self) -> u64 {
        unsafe { self.raw.via.u64_.val }
    }
});

typed_object!(WafObjectType::String => WafString
    is_valid {
        pub fn is_valid(&self) -> bool {
            self.raw.obj_type() & libddwaf_sys::DDWAF_OBJ_STRING != 0
        }
    }
    {
    /// Creates a new [`WafString`] with the provided value.
    /// Only returns none if the string is larger than [`u32::MAX`] bytes.
    ///
    /// # Panics
    /// Panics if memory allocation fails (out of memory).
    #[allow(clippy::cast_possible_truncation, clippy::items_after_statements)]
    pub fn new(val: impl AsRef<[u8]>) -> Option<Self> {
        let val = val.as_ref();
        if val.len() > (u32::MAX as usize) {
            return None;
        }

        const SMALL_STRING_SIZE: usize = 14;

        if val.len() <= SMALL_STRING_SIZE {
            let mut ss = libddwaf_sys::_ddwaf_object_small_string {
                type_: libddwaf_sys::DDWAF_OBJ_SMALL_STRING as u8,
                size: val.len() as u8,
                data: [0; 14],
            };
            let valcast = unsafe {
                std::slice::from_raw_parts(val.as_ptr().cast(), val.len())
            };
            ss.data[..valcast.len()].copy_from_slice(valcast);

            return Some(Self {
                raw: libddwaf_sys::ddwaf_object {
                    via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                        sstr: ss,
                    },
                },
            })
        }

        let ptr: *mut ::std::os::raw::c_char = if val.is_empty() {
            null_mut()
        } else {
            unsafe { no_fail_alloc(Layout::array::<::std::os::raw::c_char>(val.len()).unwrap()).cast() }
        };
        unsafe {
            std::ptr::copy_nonoverlapping(val.as_ptr(), ptr.cast(), val.len());
        }
        Some(Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    str_: libddwaf_sys::_ddwaf_object_string {
                        type_: libddwaf_sys::DDWAF_OBJ_STRING as u8,
                        size: val.len() as u32,
                        ptr,
                    },
                },
            },
        })
    }

    /// Creates a new [`WafString`] with the provided static value.
    ///
    /// # Panics
    /// Panics if the string is larger than [`u32::MAX`] bytes.
    #[allow(clippy::cast_possible_truncation)]
    pub fn new_literal(val: impl Into<&'static [u8]>) -> Self {
        let val = val.into();
        let len = u32::try_from(val.len()).expect("string is too large for this platform");

        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    str_: libddwaf_sys::_ddwaf_object_string {
                        type_: libddwaf_sys::DDWAF_OBJ_LITERAL_STRING as u8,
                        size: len,
                        ptr: val.as_ptr() as *mut _,
                    },
                },
            },
        }

    }

    /// Returns the length of this [`WafString`], in bytes.
    #[must_use]
    pub fn len(&self) -> u32 {
        if self.raw.obj_type() == libddwaf_sys::DDWAF_OBJ_SMALL_STRING {
            u32::from(unsafe { self.raw.via.sstr.size })
        } else {
            unsafe { self.raw.via.str_.size }
        }
    }

    /// Returns true if this [`WafString`] is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0u32
    }

    /// Returns a slice of the bytes from this [`WafString`].
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn as_bytes(&self) -> &[u8] {
        debug_assert!(self.is_valid());
        let len = self.len();
        if len == 0 {
            return &[];
        }

        if self.raw.obj_type() == libddwaf_sys::DDWAF_OBJ_SMALL_STRING {
            unsafe {
                std::slice::from_raw_parts(
                    self.raw.via.sstr.data.as_ptr().cast(),
                    len as usize,
                )
            }
        } else {
            debug_assert!(!unsafe{ self.raw.via.str_.ptr }.is_null());
            unsafe {
                std::slice::from_raw_parts(
                    self.raw.via.str_.ptr.cast(),
                    len as usize,
                )
            }
        }
    }

    /// Returns a string slice from this [`WafString`].
    ///
    /// # Errors
    /// Returns an error if the underlying data is not a valid UTF-8 string, under the same conditions as
    /// [`std::str::from_utf8`].
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }
});
typed_object!(WafObjectType::Array => WafArray {
    /// Creates a new [`WafArray`] with the provided size. All values in the array are initialized
    /// to an invalid [`WafObject`] instance.
    ///
    /// # Panics
    /// Panics if memory allocation fails (out of memory).
    #[must_use]
    pub fn new(nb_entries: u16) -> Self {
        let size = usize::from(nb_entries);
        let layout = Layout::array::<libddwaf_sys::ddwaf_object>(size).unwrap();
        let ptr = unsafe { no_fail_alloc(layout).cast() };
        unsafe { std::ptr::write_bytes(ptr, 0, size)};
        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    array: libddwaf_sys::_ddwaf_object_array {
                        #[allow(clippy::cast_possible_truncation)]
                        type_: libddwaf_sys::DDWAF_OBJ_ARRAY as u8,
                        size: nb_entries,
                        capacity: nb_entries,
                        ptr,
                    },
                },
            }
        }
    }

    /// Returns the length of this [`WafArray`].
    #[must_use]
    pub const fn len(&self) -> u16 {
        unsafe { self.raw.via.array.size }
    }

    /// Returns true if this [`WafArray`] is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the capacity of this [`WafArray`].
    ///
    /// The capacity is an implementation detail and is only used to for properly
    /// deallocating the memory when the array is dropped.
    #[must_use]
    pub const fn capacity(&self) -> u16 {
        unsafe { self.raw.via.array.capacity }
    }

    /// Truncates this [`WafArray`] to the provided size.
    ///
    /// Has no effect is the current length is not greater than the new size.
    ///
    /// It does not free the extra memory, except insofar as it drops the extra elements.
    /// Useful when you pessimistically allocate a larger array, but later discover that you don't need all the capacity.
    pub fn truncate(&mut self, new_size: u16) {
        if new_size > self.len() {
            return;
        }
        let arr: *mut WafObject = unsafe { self.raw.via.array.ptr.cast() };
        for i in new_size..self.len() {
            unsafe {
                std::ptr::drop_in_place(arr.add(i as usize));
            }
        }
        self.raw.via.array.size = new_size;
    }

    /// Returns an iterator over the [`Keyed<WafObject>`]s in this [`WafMap`].
    pub fn iter(&self) -> impl Iterator<Item = &WafObject> {
        let slice : &[WafObject] = self.as_ref();
        slice.iter()
    }

    /// Returns a mutable iterator over the [`Keyed<WafObject>`]s in this [`WafMap`].
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut WafObject> {
        let slice : &mut [WafObject] = AsMut::as_mut(self);
        slice.iter_mut()
    }
});
typed_object!(WafObjectType::Map => WafMap {
    /// Creates a new [`WafMap`] with the provided size. All values in the map are initialized
    /// to an invalid [`WafObject`] instance with a blank key.
    ///
    /// # Panics
    /// Panics if memory allocation fails (out of memory).
    #[must_use]
    pub fn new(nb_entries: u16) -> Self {
        let size = usize::from(nb_entries);
        let layout = Layout::array::<libddwaf_sys::_ddwaf_object_kv>(size).unwrap();
        let ptr = unsafe { no_fail_alloc(layout).cast() };
        unsafe { std::ptr::write_bytes(ptr, 0, size)};
        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    map: libddwaf_sys::_ddwaf_object_map {
                        #[allow(clippy::cast_possible_truncation)]
                        type_: libddwaf_sys::DDWAF_OBJ_MAP as u8,
                        size: nb_entries,
                        capacity: nb_entries,
                        ptr,
                    },
                },
            }
        }
    }

    /// Returns the length of this [`WafMap`].
    #[must_use]
    pub const fn len(&self) -> u16 {
        unsafe { self.raw.via.map.size }
    }

    /// Returns true if this [`WafMap`] is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the capacity of this [`WafMap`].
    ///
    /// The capacity is an implementation detail and is only used to for properly
    /// deallocating the memory when the map is dropped.
    #[must_use]
    pub const fn capacity(&self) -> u16 {
        unsafe { self.raw.via.map.capacity }
    }

    /// Truncates this [`WafMap`] to the provided size.
    ///
    /// Has no effect is the current length is not greater than the new size.
    ///
    /// It does not free the extra memory, except insofar as it drops the extra elements.
    /// Useful when you pessimistically allocate a larger map, but later discover that you don't need all the capacity.
    pub fn truncate(&mut self, new_size: u16) {
        if new_size > self.len() {
            return;
        }
        let entries: *mut Keyed<WafObject> = unsafe { self.raw.via.map.ptr.cast() };
        for i in new_size..self.len() {
            unsafe {
                std::ptr::drop_in_place(entries.add(i as usize));
            }
        }
        self.raw.via.map.size = new_size;
    }

    /// Returns an iterator over the [`Keyed<WafObject>`]s in this [`WafMap`].
    pub fn iter(&self) -> impl Iterator<Item = &Keyed<WafObject>> {
        let slice : &[Keyed<WafObject>] = self.as_ref();
        slice.iter()
    }

    /// Returns a mutable iterator over the [`Keyed<WafObject>`]s in this [`WafMap`].
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Keyed<WafObject>> {
        let slice : &mut [Keyed<WafObject>] = AsMut::as_mut(self);
        slice.iter_mut()
    }

    /// Returns a reference to the [`Keyed<WafObject>`] with the provided key, if one exists.
    ///
    /// If multiple such objects exist in the receiver, the first match is returned.
    #[must_use]
    pub fn get(&self, key: impl AsRef<libddwaf_sys::ddwaf_object>) -> Option<&Keyed<WafObject>> {
        let key = key.as_ref();
        self.iter().find(|o| o.key().raw.eq(key))
    }

    /// Returns a reference to the [`Keyed<WafObject>`] with the provided key, if one exists.
    ///
    /// If multiple such objects exist in the receiver, the first match is returned.
    #[must_use]
    pub fn get_bstr(&self, key: &'_ [u8]) -> Option<&Keyed<WafObject>> {
        self.iter().find(|o| {
            match o.key().as_type::<WafString>() {
                Some(s) => s.as_bytes() == key,
                None => false,
            }
        })
    }

    /// Returns a mutable reference to the [`Keyed<WafObject>`] with the provided key, if one exists.
    ///
    /// If multiple such objects exist in the receiver, the first match is returned.
    pub fn get_mut(&mut self, key: &'_ [u8]) -> Option<&mut Keyed<WafObject>> {
        self.iter_mut().find(|o| {
            match o.key().as_type::<WafString>() {
                Some(s) => s.as_bytes() == key,
                None => false
            }
        })
    }

    /// Returns a reference to the [`Keyed<WafObject>`] with the provided key, if one exists.
    #[must_use]
    pub fn get_str(&self, key: &'_ str) -> Option<&Keyed<WafObject>> {
        self.get_bstr(key.as_bytes())
    }

    /// Returns a mutable reference to the [`Keyed<WafObject>`] with the provided key, if one exists.
    pub fn get_str_mut(&mut self, key: &'_ str) -> Option<&mut Keyed<WafObject>> {
        self.get_mut(key.as_bytes())
    }
});
typed_object!(WafObjectType::Bool => WafBool derive(Copy, Clone) {
    /// Creates a new [`WafBool`] with the provided value.
    #[must_use]
    pub const fn new(val: bool) -> Self {
        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    b8: libddwaf_sys::_ddwaf_object_bool {
                        #[allow(clippy::cast_possible_truncation)]
                        type_: libddwaf_sys::DDWAF_OBJ_BOOL as u8,
                        val,
                    },
                },
            }
        }
    }

    /// Returns the value of this [`WafBool`].
    #[must_use]
    pub const fn value(&self) -> bool {
        unsafe { self.raw.via.b8.val }
    }
});

typed_object!(WafObjectType::Float => WafFloat derive(Copy, Clone) {
    /// Creates a new [`WafFloat`] with the provided value.
    #[must_use]
    pub const fn new(val: f64) -> Self {
        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    f64_: libddwaf_sys::_ddwaf_object_float {
                        #[allow(clippy::cast_possible_truncation)]
                        type_: libddwaf_sys::DDWAF_OBJ_FLOAT as u8,
                        val,
                    },
                },
            }
        }
    }

    /// Returns the value of this [`WafFloat`].
    #[must_use]
    pub const fn value(&self) -> f64 {
        unsafe { self.raw.via.f64_.val }
    }
});

typed_object!(WafObjectType::Null => WafNull derive(Copy, Clone) {
    /// Creates a new [`WafNull`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    u64_: libddwaf_sys::_ddwaf_object_unsigned {
                        #[allow(clippy::cast_possible_truncation)]
                        type_: libddwaf_sys::DDWAF_OBJ_NULL as u8,
                        val: 0,
                    },
                },
            }
        }
    }
}
);

impl fmt::Debug for WafSigned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", stringify!(WafSigned), self.value())
    }
}
impl From<i64> for WafSigned {
    fn from(value: i64) -> Self {
        Self::new(value)
    }
}
impl From<i32> for WafSigned {
    fn from(value: i32) -> Self {
        Self::new(value.into())
    }
}

impl fmt::Debug for WafUnsigned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", stringify!(WafUnsigned), self.value())
    }
}
impl From<u64> for WafUnsigned {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}
impl From<u32> for WafUnsigned {
    fn from(value: u32) -> Self {
        Self::new(value.into())
    }
}

impl<T: AsRef<[u8]>> From<T> for WafString {
    fn from(val: T) -> Self {
        let slice = val.as_ref();
        let slice = &slice[..slice.len().min(u32::MAX as usize)];
        Self::new(slice).unwrap()
    }
}
impl fmt::Debug for WafString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}(\"{:?}\")",
            stringify!(WafString),
            fmt_bin_str(self.as_bytes())
        )
    }
}
impl Drop for WafString {
    fn drop(&mut self) {
        // Only call drop_string for heap-allocated strings (DDWAF_OBJ_STRING)
        // LITERAL_STRING (static) and SMALL_STRING (inline) don't need deallocation
        if self.raw.obj_type() == libddwaf_sys::DDWAF_OBJ_STRING {
            unsafe { self.raw.drop_string() }
        }
    }
}
impl Clone for WafString {
    fn clone(&self) -> Self {
        if self.raw.obj_type() == libddwaf_sys::DDWAF_OBJ_STRING {
            let len = self.len();
            let layout = Layout::array::<std::os::raw::c_char>(len as usize).unwrap();
            let copied = unsafe { no_fail_alloc(layout).cast::<std::os::raw::c_char>() };
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.as_bytes().as_ptr().cast(),
                    copied,
                    len as usize,
                );
            }
            return Self {
                raw: libddwaf_sys::ddwaf_object {
                    via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                        str_: libddwaf_sys::_ddwaf_object_string {
                            #[allow(clippy::cast_possible_truncation)]
                            type_: libddwaf_sys::DDWAF_OBJ_STRING as u8,
                            size: len,
                            ptr: copied,
                        },
                    },
                },
            };
        }

        // other string types, just a plain copy
        Self { raw: self.raw }
    }
}

impl AsRef<[WafObject]> for WafArray {
    fn as_ref(&self) -> &[WafObject] {
        if self.is_empty() {
            return &[];
        }
        let array = unsafe { self.raw.via.array.ptr.cast() };
        unsafe { std::slice::from_raw_parts(array, self.len() as usize) }
    }
}
impl AsMut<[WafObject]> for WafArray {
    fn as_mut(&mut self) -> &mut [WafObject] {
        if self.is_empty() {
            return &mut [];
        }
        let array = unsafe { self.raw.via.array.ptr.cast() };
        unsafe { std::slice::from_raw_parts_mut(array, self.len() as usize) }
    }
}
impl fmt::Debug for WafArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[", stringify!(WafArray))?;
        let mut first = true;
        for obj in self.iter() {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{obj:?}")?;
        }
        write!(f, "]")
    }
}
impl Drop for WafArray {
    fn drop(&mut self) {
        unsafe { self.raw.drop_array() }
    }
}
impl Clone for WafArray {
    fn clone(&self) -> Self {
        let size = self.len();

        if size == 0 {
            return Self::new(0);
        }

        let layout = Layout::array::<libddwaf_sys::ddwaf_object>(size as usize).unwrap();
        let new_arr: *mut libddwaf_sys::ddwaf_object = unsafe { no_fail_alloc(layout).cast() };

        // Clone each element
        for i in 0..size {
            let src_elem: &WafObject = &self[i as usize];
            let cloned_elem = ManuallyDrop::new(src_elem.clone());
            unsafe { new_arr.add(i as usize).write(cloned_elem.raw) };
        }

        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    array: libddwaf_sys::_ddwaf_object_array {
                        #[allow(clippy::cast_possible_truncation)]
                        type_: libddwaf_sys::DDWAF_OBJ_ARRAY as u8,
                        size,
                        capacity: size,
                        ptr: new_arr,
                    },
                },
            },
        }
    }
}
impl<T: Into<WafObject>, const N: usize> From<[T; N]> for WafArray {
    fn from(value: [T; N]) -> Self {
        let effective_length = N.min(u16::MAX as usize);
        #[allow(clippy::cast_possible_truncation)]
        let mut array = Self::new(effective_length as u16);
        for (i, obj) in value.into_iter().enumerate() {
            if i >= effective_length {
                break;
            }
            array[i] = obj.into();
        }
        array
    }
}
impl<T> From<&mut [T]> for WafArray
where
    T: Into<WafObject> + Default,
{
    fn from(value: &mut [T]) -> Self {
        let effective_length = value.len().min(u16::MAX as usize);
        #[allow(clippy::cast_possible_truncation)]
        let mut array = Self::new(effective_length as u16);
        for (i, obj) in value.iter_mut().enumerate() {
            if i >= effective_length {
                break;
            }
            let obj = std::mem::take(obj);
            array[i] = obj.into();
        }
        array
    }
}
impl Index<usize> for WafArray {
    type Output = WafObject;
    fn index(&self, index: usize) -> &Self::Output {
        let len = self.len() as usize;
        assert!(index < len, "index out of bounds ({index} >= {len})");
        let array = unsafe { self.raw.via.array.ptr };
        unsafe { &*(array.add(index) as *const _) }
    }
}
impl IndexMut<usize> for WafArray {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let len = self.len() as usize;
        assert!(index < len, "index out of bounds ({index} >= {len})");
        let array = unsafe { self.raw.via.array.ptr };
        unsafe { &mut *(array.add(index).cast()) }
    }
}

impl AsRef<[Keyed<WafObject>]> for WafMap {
    fn as_ref(&self) -> &[Keyed<WafObject>] {
        if self.is_empty() {
            return &[];
        }
        let ptr = unsafe { self.raw.via.map.ptr as *const _ };
        unsafe { std::slice::from_raw_parts(ptr, self.len() as usize) }
    }
}
impl AsMut<[Keyed<WafObject>]> for WafMap {
    fn as_mut(&mut self) -> &mut [Keyed<WafObject>] {
        if self.is_empty() {
            return &mut [];
        }
        let ptr = unsafe { self.raw.via.map.ptr.cast() };
        unsafe { std::slice::from_raw_parts_mut(ptr, self.len() as usize) }
    }
}
impl fmt::Debug for WafMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{{", stringify!(WafMap))?;
        let mut first = true;
        for keyed_obj in self.iter() {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{keyed_obj:?}")?;
        }
        write!(f, "}}")
    }
}
impl Drop for WafMap {
    fn drop(&mut self) {
        unsafe { self.raw.drop_map() }
    }
}
impl Clone for WafMap {
    fn clone(&self) -> Self {
        let size = self.len();

        if size == 0 {
            return Self::new(0);
        }

        let layout = Layout::array::<libddwaf_sys::_ddwaf_object_kv>(size as usize).unwrap();
        let new_ptr: *mut libddwaf_sys::_ddwaf_object_kv = unsafe { no_fail_alloc(layout).cast() };
        unsafe { std::ptr::write_bytes(new_ptr, 0, size as usize) };

        // Clone each key-value pair
        for i in 0..size {
            let src_entry: &Keyed<WafObject> = &self[i as usize];
            let cloned_entry = ManuallyDrop::new(src_entry.clone());
            unsafe { new_ptr.add(i as usize).write(cloned_entry.raw) };
        }

        Self {
            raw: libddwaf_sys::ddwaf_object {
                via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                    map: libddwaf_sys::_ddwaf_object_map {
                        #[allow(clippy::cast_possible_truncation)]
                        type_: libddwaf_sys::DDWAF_OBJ_MAP as u8,
                        size,
                        capacity: size,
                        ptr: new_ptr,
                    },
                },
            },
        }
    }
}
impl Index<usize> for WafMap {
    type Output = Keyed<WafObject>;
    fn index(&self, index: usize) -> &Self::Output {
        let len = self.len() as usize;
        assert!(index < len, "index out of bounds ({index} >= {len})");
        let ptr = unsafe { self.raw.via.map.ptr };
        unsafe { &*ptr.add(index).cast() }
    }
}
impl IndexMut<usize> for WafMap {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let len = self.len() as usize;
        assert!(index < len, "index out of bounds ({index} >= {len})");
        let ptr = unsafe { self.raw.via.map.ptr };
        unsafe { &mut *ptr.add(index).cast() }
    }
}
impl<K: AsRef<[u8]>, V: Into<WafObject>, const N: usize> From<[(K, V); N]> for WafMap {
    fn from(vals: [(K, V); N]) -> Self {
        let effective_length = N.min(u16::MAX as usize);
        #[allow(clippy::cast_possible_truncation)]
        let mut map = WafMap::new(effective_length as u16);
        for (i, (k, v)) in vals.into_iter().enumerate() {
            if i >= effective_length {
                break;
            }
            map[i] = Keyed::from((k.as_ref(), v.into()));
        }
        map
    }
}
impl<V: Into<WafObject>, const N: usize> From<[(WafObject, V); N]> for WafMap {
    fn from(vals: [(WafObject, V); N]) -> Self {
        let effective_length = N.min(u16::MAX as usize);
        #[allow(clippy::cast_possible_truncation)]
        let mut map = WafMap::new(effective_length as u16);
        for (i, (k, v)) in vals.into_iter().enumerate() {
            if i >= effective_length {
                break;
            }
            map[i] = (k, v.into()).into();
        }
        map
    }
}
impl<K, V> From<&mut [(K, V)]> for WafMap
where
    K: Into<WafObject> + Default,
    V: Into<WafObject> + Default,
{
    fn from(value: &mut [(K, V)]) -> Self {
        let effective_length = value.len().min(u16::MAX as usize);
        #[allow(clippy::cast_possible_truncation)]
        let mut map = Self::new(effective_length as u16);
        for (i, (k, v)) in value.iter_mut().enumerate() {
            if i >= effective_length {
                break;
            }
            let k = std::mem::take(k);
            let v = std::mem::take(v);
            map[i] = (k.into(), v.into()).into();
        }
        map
    }
}

impl fmt::Debug for WafBool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", stringify!(WafBool), self.value())
    }
}
impl From<bool> for WafBool {
    fn from(value: bool) -> Self {
        Self::new(value)
    }
}

impl fmt::Debug for WafFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", stringify!(WafFloat), self.value())
    }
}
impl From<f64> for WafFloat {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

impl fmt::Debug for WafNull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", stringify!(WafNull))
    }
}
impl From<()> for WafNull {
    fn from((): ()) -> Self {
        Self::new()
    }
}

/// An [`WafObject`] or [`TypedWafObject`] associated with a key.
#[repr(transparent)]
pub struct Keyed<T: AsRawMutObject> {
    raw: libddwaf_sys::_ddwaf_object_kv,
    _marker: std::marker::PhantomData<T>,
}
impl<T: AsRawMutObject> Keyed<T> {
    /// Creates a new [`Keyed<WafObject>`] with the provided key and value.
    pub fn new(key: impl Into<WafObject>, value: T) -> Self {
        let key = key.into();
        let val = *value.as_ref();
        let ret = Self {
            raw: libddwaf_sys::_ddwaf_object_kv { key: key.raw, val },
            _marker: std::marker::PhantomData,
        };
        std::mem::forget(key);
        std::mem::forget(value);
        ret
    }

    // Obtains a reference to the map entry key.
    #[must_use]
    pub fn key(&self) -> &WafObject {
        unsafe { self.raw.key.unchecked_as_ref() }
    }

    /// Obtains a mutable reference to the map entry key.
    #[must_use]
    pub fn key_mut(&mut self) -> &mut WafObject {
        unsafe { self.raw.key.unchecked_as_ref_mut() }
    }

    /// Obtains a reference to the map entry value.
    #[must_use]
    pub fn value(&self) -> &T {
        unsafe { self.raw.val.unchecked_as_ref() }
    }

    /// Obtains a mutable reference to the map entry value.
    #[must_use]
    pub fn value_mut(&mut self) -> &mut T {
        unsafe { self.raw.val.unchecked_as_ref_mut() }
    }

    /// Obtains the key associated with this [`Keyed<WafObject>`] as a string.
    ///
    /// # Errors
    /// Returns an error if the underlying key data is not a valid UTF-8 string, under the same conditions as
    /// [`std::str::from_utf8`] or if the key is not a [`WafString`].
    #[allow(invalid_from_utf8)]
    pub fn key_str(&self) -> Result<&str, Box<dyn std::error::Error>> {
        std::str::from_utf8(self.key_bytes()?).map_err(std::convert::Into::into)
    }

    /// Obtains the key associated with this [`Keyed<WafObject>`] as a byte slice.
    ///
    /// # Errors
    /// Returns an error if the underlying key data is not a [`WafString`].
    pub fn key_bytes(&self) -> Result<&[u8], ObjectTypeError> {
        let key = self.key();
        match key.as_type::<WafString>() {
            Some(s) => Ok(s.as_bytes()),
            None => Err(ObjectTypeError {
                expected: WafObjectType::String,
                actual: key.object_type(),
            }),
        }
    }
}
impl Keyed<WafObject> {
    #[must_use]
    pub fn as_type<T: TypedWafObject>(&self) -> Option<&Keyed<T>> {
        if self.value().object_type() == T::TYPE {
            Some(unsafe { &*(std::ptr::from_ref(self).cast()) })
        } else {
            None
        }
    }

    pub fn as_type_mut<T: TypedWafObject>(&mut self) -> Option<&mut Keyed<T>> {
        if self.value().object_type() == T::TYPE {
            Some(unsafe { &mut *(std::ptr::from_mut(self).cast()) })
        } else {
            None
        }
    }
}
// Note - We are not implementing DerefMut for Keyed as it'd allow leaking the key if it is used
// through [std::mem::take] or [std::mem::replace].
impl Keyed<WafArray> {
    pub fn iter(&self) -> impl Iterator<Item = &WafObject> {
        self.value().iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut WafObject> {
        self.value_mut().iter_mut()
    }
}
// Note - We are not implementing DerefMut for Keyed as it'd allow leaking the key if it is used
// through [std::mem::take] or [std::mem::replace].
impl Keyed<WafMap> {
    pub fn iter(&self) -> impl Iterator<Item = &Keyed<WafObject>> {
        self.value().iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Keyed<WafObject>> {
        self.value_mut().iter_mut()
    }
}
// impl<T: AsRawMutObject> AsRawMutObject for Keyed<T> {
//     unsafe fn as_raw_mut(&mut self) -> &mut libddwaf_sys::ddwaf_object {
//         unsafe { self.value_mut().as_raw_mut() }
//     }
// }
impl<T: AsRawMutObject> crate::private::Sealed for Keyed<T> {}
impl<T: AsRawMutObject> AsRef<libddwaf_sys::_ddwaf_object_kv> for Keyed<T> {
    fn as_ref(&self) -> &libddwaf_sys::_ddwaf_object_kv {
        &self.raw
    }
}
impl<T: Default + AsRawMutObject> std::default::Default for Keyed<T> {
    fn default() -> Self {
        let key = WafObject::default();
        let mut value = T::default();
        let ret = Self {
            raw: libddwaf_sys::_ddwaf_object_kv {
                key: key.raw,
                val: *unsafe { value.as_raw_mut() },
            },
            _marker: std::marker::PhantomData,
        };
        std::mem::forget(key);
        std::mem::forget(value);
        ret
    }
}
impl<T: AsRawMutObject> Deref for Keyed<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.value()
    }
}
impl<T: AsRawMutObject> std::ops::Drop for Keyed<T> {
    fn drop(&mut self) {
        unsafe { self.raw.key.drop_object() };
        unsafe { self.raw.val.drop_object() };
    }
}
impl<T: AsRawMutObject + fmt::Debug> fmt::Debug for Keyed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let k = self.key();
        if k.object_type() == WafString::TYPE {
            write!(
                f,
                "\"{:?}\"={:?}",
                fmt_bin_str(unsafe { self.key().as_type_unchecked::<WafString>() }.as_bytes()),
                self.value()
            )
        } else {
            write!(f, "{:?}={:?}", k, self.value())
        }
    }
}
impl<T, U: AsRawMutObject> From<(&str, T)> for Keyed<U>
where
    T: Into<U>,
{
    fn from(value: (&str, T)) -> Self {
        (value.0.as_bytes(), value.1).into()
    }
}
impl<T, U: AsRawMutObject> From<(&[u8], T)> for Keyed<U>
where
    T: Into<U>,
{
    fn from(value: (&[u8], T)) -> Self {
        let key: WafObject = value.0.into();
        let value: U = value.1.into();
        Keyed::new(key, value)
    }
}
impl<T: TypedWafObject> From<Keyed<T>> for Keyed<WafObject> {
    fn from(value: Keyed<T>) -> Self {
        let res = Self {
            raw: value.raw,
            _marker: std::marker::PhantomData,
        };
        std::mem::forget(value);
        res
    }
}
impl From<(WafObject, WafObject)> for Keyed<WafObject> {
    fn from(value: (WafObject, WafObject)) -> Self {
        Keyed::new(value.0, value.1)
    }
}
impl<T: TypedWafObject> From<(WafObject, T)> for Keyed<T> {
    fn from(value: (WafObject, T)) -> Self {
        Keyed::new(value.0, value.1)
    }
}
impl<T: AsRawMutObject + Clone> Clone for Keyed<T> {
    fn clone(&self) -> Self {
        let cloned_key = self.key().clone();
        let cloned_value = self.value().clone();

        let ret = Self {
            raw: libddwaf_sys::_ddwaf_object_kv {
                key: cloned_key.raw,
                val: *cloned_value.as_ref(),
            },
            _marker: std::marker::PhantomData,
        };

        std::mem::forget(cloned_key);
        std::mem::forget(cloned_value);
        ret
    }
}
trait UncheckedAsRef: crate::private::Sealed {
    /// Converts a naked reference to a [`libddwaf_sys::ddwaf_object`] into a reference to one of the
    /// user-friendlier types.
    ///
    /// # Safety
    /// The type `T` must be able to represent this [`libddwaf_sys::ddwaf_object`]'s type (per its
    /// associated [`libddwaf_sys::DDWAF_OBJ_TYPE`] value).
    unsafe fn unchecked_as_ref<T: AsRef<libddwaf_sys::ddwaf_object> + crate::private::Sealed>(
        &self,
    ) -> &T;

    /// Converts a naked mutable reference to a `ddwaf_object` into a mutable reference to one of the
    ///
    /// # Safety
    /// - The type `T` must be able to represent this [`libddwaf_sys::ddwaf_object`]'s type (per its
    ///   associated [`libddwaf_sys::DDWAF_OBJ_TYPE`] value).
    /// - The destructor of `T` must be compatible with the value of self.
    unsafe fn unchecked_as_ref_mut<T: AsRef<libddwaf_sys::ddwaf_object> + crate::private::Sealed>(
        &mut self,
    ) -> &mut T;
}
impl crate::private::Sealed for libddwaf_sys::ddwaf_object {}
impl UncheckedAsRef for libddwaf_sys::ddwaf_object {
    unsafe fn unchecked_as_ref<T: AsRef<libddwaf_sys::ddwaf_object> + crate::private::Sealed>(
        &self,
    ) -> &T {
        unsafe { &*(std::ptr::from_ref(self).cast()) }
    }

    unsafe fn unchecked_as_ref_mut<
        T: AsRef<libddwaf_sys::ddwaf_object> + crate::private::Sealed,
    >(
        &mut self,
    ) -> &mut T {
        unsafe { &mut *(std::ptr::from_mut(self).cast()) }
    }
}
trait UncheckedAsWafObject: crate::private::Sealed {
    /// Converts a naked reference to a [`libddwaf_sys::ddwaf_object`] into a reference to an [`WafObject`].
    fn as_object_ref(&self) -> &WafObject;
}
impl<T: UncheckedAsRef> UncheckedAsWafObject for T {
    /// Converts a naked reference to a [`libddwaf_sys::ddwaf_object`] into a reference to an [`WafObject`].
    fn as_object_ref(&self) -> &WafObject {
        unsafe { self.unchecked_as_ref::<WafObject>() }
    }
}

/// Formats a byte slice as an ASCII string, hex-escaping any non-printable characters.
fn fmt_bin_str(bytes: &[u8]) -> impl fmt::Debug + '_ {
    struct BinFormatter<'a>(&'a [u8]);
    impl fmt::Debug for BinFormatter<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for &c in self.0 {
                if c == b'"' || c == b'\\' {
                    write!(f, "\\{}", c as char)?;
                } else if c.is_ascii_graphic() || c == b' ' {
                    write!(f, "{}", c as char)?;
                } else {
                    write!(f, "\\x{c:02X}")?;
                }
            }
            Ok(())
        }
    }
    BinFormatter(bytes)
}

pub(crate) struct RustDdwafAllocator {
    raw: libddwaf_sys::ddwaf_allocator,
}
impl RustDdwafAllocator {
    fn new() -> Option<Self> {
        let allocator = unsafe {
            libddwaf_sys::ddwaf_user_allocator_init(
                Some(Self::alloc_fn),
                Some(Self::free_fn),
                std::ptr::null_mut(),
                Option::None,
            )
        };
        if allocator.is_null() {
            None
        } else {
            Some(Self { raw: allocator })
        }
    }
    extern "C" fn alloc_fn(
        _udata: *mut ::std::os::raw::c_void,
        size: usize,
        alignment: usize,
    ) -> *mut ::std::os::raw::c_void {
        let layout = Layout::from_size_align(size, alignment);
        if let Ok(layout) = layout {
            unsafe { std::alloc::alloc(layout).cast() }
        } else {
            debug_assert!(false, "Invalid layout");
            std::ptr::null_mut()
        }
    }

    extern "C" fn free_fn(
        _udata: *mut ::std::os::raw::c_void,
        ptr: *mut ::std::os::raw::c_void,
        size: usize,
        alignment: usize,
    ) {
        let layout = Layout::from_size_align(size, alignment);
        match layout {
            Ok(layout) => unsafe { std::alloc::dealloc(ptr.cast(), layout) },
            Err(_) => {
                debug_assert!(false, "Invalid layout");
            }
        }
    }
}

impl Drop for RustDdwafAllocator {
    fn drop(&mut self) {
        unsafe { libddwaf_sys::ddwaf_allocator_destroy(self.raw) };
    }
}

impl From<&RustDdwafAllocator> for libddwaf_sys::ddwaf_allocator {
    fn from(allocator: &RustDdwafAllocator) -> Self {
        allocator.raw
    }
}

// RustDdwafAllocator is immutable
unsafe impl Sync for RustDdwafAllocator {}
unsafe impl Send for RustDdwafAllocator {}

static DEFAULT_ALLOCATOR: OnceLock<RustDdwafAllocator> = OnceLock::new();

pub(crate) fn get_default_allocator() -> &'static RustDdwafAllocator {
    DEFAULT_ALLOCATOR.get_or_init(|| RustDdwafAllocator::new().unwrap())
}

/// Helper macro to create [`WafObject`]s.
#[macro_export]
macro_rules! waf_object {
    (null) => {
        $crate::object::WafObject::from(())
    };
    ($l:expr) => {
        $crate::object::WafObject::from($l)
    };
}

/// Helper macro to create [`WafArray`]s.
#[macro_export]
macro_rules! waf_array {
    () => { $crate::object::WafArray::new(0) };
    ($($e:expr),* $(,)?) => {
        {
            let size = [$($crate::__repl_expr_with_unit!($e)),*].len();
            let mut res = $crate::object::WafArray::new(size as u16);
            let mut i = usize::MAX;
            $(
                i = i.wrapping_add(1);
                res[i] = $crate::waf_object!($e);
            )*
            res
        }
    };
}

/// Helper macro to create [`WafMap`]s.
#[macro_export]
macro_rules! waf_map {
    () => { $crate::object::WafMap::new(0) };
    ($(($k:literal, $v:expr)),* $(,)?) => {
        {
            let size = [$($crate::__repl_expr_with_unit!($v)),*].len();
            let mut res = $crate::object::WafMap::new(u16::try_from(size).unwrap());
            let mut i = usize::MAX;
            $(
                i = i.wrapping_add(1);
                let k = $crate::object::WafString::new_literal($k.as_bytes());
                let val: $crate::object::WafObject = $v.into();
                res[i] = $crate::object::Keyed::new(k, val);
            )*
            res
        }
    };
}

/// Helper macro to facilitate counting token trees within other macros.
///
/// Not intended for use outside of this crate, but must be exported as it is used by macros in this crate.
#[doc(hidden)]
#[macro_export]
macro_rules! __repl_expr_with_unit {
    ($e:expr) => {
        ()
    };
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    #[allow(clippy::float_cmp)] // No operations are done on the values, they should be the same.
    #[allow(clippy::cast_possible_truncation)]
    fn unsafe_changes_to_default_objects() {
        unsafe {
            let mut unsigned = WafUnsigned::default();
            unsigned.as_raw_mut().via.u64_.val += 1;
            assert_eq!(unsigned.value(), 1);

            let mut signed = WafSigned::default();
            signed.as_raw_mut().via.i64_.val -= 1;
            assert_eq!(signed.value(), -1);

            let mut float = WafFloat::default();
            float.as_raw_mut().via.f64_.val += 1.0;
            assert_eq!(float.value(), 1.0);

            let mut boolean = WafBool::default();
            boolean.as_raw_mut().via.b8.val = true;
            assert!(boolean.value());

            let null = WafNull::default();
            // nothing interesting to do for null; let's try manually setting
            // the parameter name
            let s = String::from_str("foobar").unwrap();
            let keyed_null = Keyed::new(WafString::from(s.as_str()), null);
            std::mem::drop(keyed_null);

            let mut string = WafString::default();
            let str_mut = string.as_raw_mut();
            let p: *mut u8 =
                no_fail_alloc(Layout::array::<::std::os::raw::c_char>(s.len()).unwrap()).cast();
            std::ptr::copy_nonoverlapping(s.as_ptr(), p.cast(), s.len());
            str_mut.drop_string();
            str_mut.via.str_.ptr = p.cast();
            str_mut.via.str_.size = s.len() as u32;
            assert_eq!(string.as_str().unwrap(), "foobar");
            assert_eq!(string.len(), s.len() as u32);
            assert!(!string.is_empty());
        }
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn string_representations_are_equivalent() {
        const HELLO: &[u8] = b"hello";

        let ss = WafString::new("hello").unwrap();
        assert_eq!(ss.raw.obj_type(), libddwaf_sys::DDWAF_OBJ_SMALL_STRING);
        assert!(ss.is_valid());
        assert!(ss.raw.is_string());

        let ls = WafString::new_literal(HELLO);
        assert_eq!(ls.raw.obj_type(), libddwaf_sys::DDWAF_OBJ_LITERAL_STRING);
        assert!(ls.is_valid());
        assert!(ls.raw.is_string());
        assert_eq!(ss, ls);

        let ns = libddwaf_sys::ddwaf_object {
            via: libddwaf_sys::_ddwaf_object__bindgen_ty_1 {
                str_: libddwaf_sys::_ddwaf_object_string {
                    type_: libddwaf_sys::DDWAF_OBJ_STRING as u8,
                    size: HELLO.len() as u32,
                    ptr: HELLO.as_ptr() as *mut _,
                },
            },
        };
        let ns = unsafe { ns.unchecked_as_ref::<WafString>() };
        assert_eq!(ns.raw.obj_type(), libddwaf_sys::DDWAF_OBJ_STRING);
        assert!(ns.is_valid());
        assert!(ns.raw.is_string());
        assert_eq!(ns.as_bytes(), HELLO);
        assert_eq!(*ns, ss);
        assert_eq!(*ns, ls);
    }
}
