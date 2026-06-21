use std::error;
use std::fmt;
use std::time::Duration;

use crate::object::WafOwnedOutputAllocator;
use crate::object::{
    AllocatorType, AsRawMutObject, Keyed, RustAllocator, WafArray, WafMap, WafObject, WafOwned,
};

/// A WAF Context that can be used to evaluate the configured ruleset against address data.
///
/// This is obtained by calling [`Handle::new_context`][crate::Handle::new_context], and a given [`Context`] should only
/// be used to handle data for a single request.
pub struct Context {
    pub(crate) raw: libddwaf_sys::ddwaf_context,
}

/// Subcontexts are type of [`Context`] that inherit the data from their parents,
/// but evaluations do not affect the parent's data.
///
/// Subcontexts can outlive their parent contexts.
///
/// They are obtained by calling [`Context::new_subcontext`][crate::Context::new_subcontext].
pub struct Subcontext {
    pub(crate) raw: libddwaf_sys::ddwaf_subcontext,
}

/// Common waf evaluation interface for [`Context`] and [`Subcontext`].
pub trait RunnableContext {
    /// Evaluates the configured ruleset against the provided address data, and returns the result
    /// of this evaluation.
    ///
    /// # Errors
    /// Returns an error if the WAF encountered an internal error, invalid object, or invalid argument while processing
    /// the request.
    fn run(&mut self, data: WafMap, timeout: Duration) -> Result<RunResult, RunError>;

    /// Evaluates allocator-owned address data, using the allocator encoded by `data` when
    /// transferring ownership to libddwaf.
    ///
    /// # Errors
    /// Returns an error if the WAF encountered an internal error, invalid object, or invalid argument while processing
    /// the request.
    fn run_owned<A: AllocatorType>(
        &mut self,
        data: WafOwned<WafObject, A>,
        timeout: Duration,
    ) -> Result<RunResult, RunError>;

    /// Evaluates multiple batches of address data in sequence, and returns a combined result.
    ///
    /// Each element in `data` must be a [`WafMap`] containing address data. Addresses from earlier
    /// batches remain available while later batches are evaluated.
    ///
    /// # Errors
    /// Returns an error if the WAF encountered an internal error, invalid object, or invalid argument while processing
    /// the request.
    fn run_batches(&mut self, data: WafArray, timeout: Duration) -> Result<RunResult, RunError>;
}

type RunFunc<S> = unsafe extern "C" fn(
    S,
    *mut libddwaf_sys::ddwaf_object,
    libddwaf_sys::ddwaf_allocator,
    *mut libddwaf_sys::ddwaf_object,
    u64,
) -> libddwaf_sys::DDWAF_RET_CODE;

fn run<S>(
    raw_self: S,
    func: RunFunc<S>,
    func_name: &'static str,
    mut data: impl AsRawMutObject,
    allocator: libddwaf_sys::ddwaf_allocator,
    timeout: Duration,
) -> Result<RunResult, RunError> {
    let mut res = std::mem::MaybeUninit::<RunOutput>::uninit();

    let data_ptr = unsafe { data.as_raw_mut() };

    let status = unsafe {
        func(
            raw_self,
            data_ptr,
            allocator,
            res.as_mut_ptr().cast(),
            timeout.as_micros().try_into().unwrap_or(u64::MAX),
        )
    };
    match status {
        libddwaf_sys::DDWAF_ERR_INTERNAL => {
            // It's unclear whether the persistent data needs to be kept alive or not, so we
            // keep it alive to be on the safe side.
            std::mem::forget(data);
            Err(RunError::InternalError)
        }
        libddwaf_sys::DDWAF_ERR_INVALID_OBJECT => {
            // The C API frees invalid object data using the allocator passed above.
            std::mem::forget(data);
            Err(RunError::InvalidObject)
        }
        libddwaf_sys::DDWAF_ERR_INVALID_ARGUMENT => Err(RunError::InvalidArgument),
        libddwaf_sys::DDWAF_OK => {
            // We need to keep the persistent data alive (now owned by the WAF)
            std::mem::forget(data);
            Ok(RunResult::NoMatch(unsafe { res.assume_init() }))
        }
        libddwaf_sys::DDWAF_MATCH => {
            // We need to keep the persistent data alive (now owned by the WAF)
            std::mem::forget(data);
            Ok(RunResult::Match(unsafe { res.assume_init() }))
        }
        unknown => unreachable!(
            "Unexpected value returned by {}: 0x{:02X}",
            func_name,
            unknown
        ),
    }
}
impl RunnableContext for Context {
    fn run(&mut self, data: WafMap, timeout: Duration) -> Result<RunResult, RunError> {
        run(
            self.raw,
            libddwaf_sys::ddwaf_context_eval,
            stringify!(libddwaf_sys::ddwaf_context_eval),
            data,
            RustAllocator::allocator(),
            timeout,
        )
    }

    fn run_owned<A: AllocatorType>(
        &mut self,
        data: WafOwned<WafObject, A>,
        timeout: Duration,
    ) -> Result<RunResult, RunError> {
        run(
            self.raw,
            libddwaf_sys::ddwaf_context_eval,
            stringify!(libddwaf_sys::ddwaf_context_eval),
            data,
            A::allocator(),
            timeout,
        )
    }

    fn run_batches(&mut self, data: WafArray, timeout: Duration) -> Result<RunResult, RunError> {
        run(
            self.raw,
            libddwaf_sys::ddwaf_context_multieval,
            stringify!(libddwaf_sys::ddwaf_context_multieval),
            data,
            RustAllocator::allocator(),
            timeout,
        )
    }
}
impl Context {
    /// Creates a new [`Subcontext`] from this [`Context`].
    ///
    /// # Errors
    /// Returns an error if the WAF encountered an internal error while creating the subcontext.
    /// This will not happen unless there is a bug in the WAF.
    pub fn new_subcontext(&self) -> Result<Subcontext, InternalError> {
        let raw = unsafe { libddwaf_sys::ddwaf_subcontext_init(self.raw) };
        if raw.is_null() {
            Err(InternalError {})
        } else {
            Ok(Subcontext { raw })
        }
    }
}
impl RunnableContext for Subcontext {
    fn run(&mut self, data: WafMap, timeout: Duration) -> Result<RunResult, RunError> {
        run(
            self.raw,
            libddwaf_sys::ddwaf_subcontext_eval,
            stringify!(libddwaf_sys::ddwaf_subcontext_eval),
            data,
            RustAllocator::allocator(),
            timeout,
        )
    }

    fn run_owned<A: AllocatorType>(
        &mut self,
        data: WafOwned<WafObject, A>,
        timeout: Duration,
    ) -> Result<RunResult, RunError> {
        run(
            self.raw,
            libddwaf_sys::ddwaf_subcontext_eval,
            stringify!(libddwaf_sys::ddwaf_subcontext_eval),
            data,
            A::allocator(),
            timeout,
        )
    }

    fn run_batches(&mut self, data: WafArray, timeout: Duration) -> Result<RunResult, RunError> {
        run(
            self.raw,
            libddwaf_sys::ddwaf_subcontext_multieval,
            stringify!(libddwaf_sys::ddwaf_subcontext_multieval),
            data,
            RustAllocator::allocator(),
            timeout,
        )
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        unsafe { libddwaf_sys::ddwaf_context_destroy(self.raw) }
    }
}
impl Drop for Subcontext {
    fn drop(&mut self) {
        unsafe { libddwaf_sys::ddwaf_subcontext_destroy(self.raw) }
    }
}

/// Safety: [`Context`] is [`Send`] because it doesn't depend on thread local
/// data and its pointer is not leaked or otherwise shared with other owning
/// instances.
unsafe impl Send for Context {}
/// Safety: [`Context`] is [`Sync`] because having a shared reference does not
/// allow for changing state (except for atomically increasing reference count
/// of some elements in the context, like that of the context store)
unsafe impl Sync for Context {}

/// Safety: The same considerations apply to [`Subcontext`] as to [`Context`].
unsafe impl Send for Subcontext {}
/// Safety: The only method available takes an exclusive borrow.
unsafe impl Sync for Subcontext {}

/// The result of the [`RunnableContext::run`] operation.
#[derive(Debug)]
pub enum RunResult {
    /// The WAF successfully processed the request, and produced no match.
    NoMatch(RunOutput),
    /// The WAF successfully processed the request and some event rules matched
    /// some of the supplied address data.
    Match(RunOutput),
}

/// The error that can occur during a [`RunnableContext::run`] operation.
#[non_exhaustive]
#[derive(Debug)]
pub enum RunError {
    /// The WAF encountered an internal error while processing the request.
    InternalError,
    /// The WAF encountered an invalid object while processing the request.
    InvalidObject,
    /// The WAF encountered an invalid argument while processing the request.
    InvalidArgument,
}
impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::InternalError => write!(f, "The WAF encountered an internal error"),
            RunError::InvalidObject => write!(f, "The WAF encountered an invalid object"),
            RunError::InvalidArgument => write!(f, "The WAF encountered an invalid argument"),
        }
    }
}
impl error::Error for RunError {}

/// An unexpected internal error in the WAF from functions other than [`RunnableContext::run`].
#[derive(Debug)]
pub struct InternalError {}
impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "An unexpected internal error occurred in the WAF; check the error logs"
        )
    }
}
impl error::Error for InternalError {}

/// The data produced by a [`Context::run`] operation.
#[repr(transparent)]
pub struct RunOutput {
    data: WafOwnedOutputAllocator<WafMap>,
}
impl RunOutput {
    /// Returns true if the WAF did not have enough time to process all the address data that was
    /// being evaluated.
    #[must_use]
    pub fn timeout(&self) -> bool {
        debug_assert!(self.data.is_valid());
        self.data
            .get_bstr(b"timeout")
            .and_then(|o| o.to_bool())
            .unwrap_or_default()
    }

    /// Returns true if the WAF determined the trace for this request should have its priority
    /// overridden to ensure it is not dropped by the sampler.
    #[must_use]
    pub fn keep(&self) -> bool {
        debug_assert!(self.data.is_valid());
        self.data
            .get_bstr(b"keep")
            .and_then(|o| o.to_bool())
            .unwrap_or_default()
    }

    /// Returns the total time spent processing the request; excluding bindings overhead (which
    /// ought to be trivial).
    pub fn duration(&self) -> Duration {
        debug_assert!(self.data.is_valid());
        self.data
            .get_bstr(b"duration")
            .and_then(|o| o.to_u64())
            .map(Duration::from_nanos)
            .unwrap_or_default()
    }

    /// Returns the number of input batches fully evaluated by the WAF.
    #[must_use]
    pub fn evaluated(&self) -> u64 {
        debug_assert!(self.data.is_valid());
        self.data
            .get_bstr(b"evaluated")
            .and_then(|o| o.to_u64())
            .unwrap_or_default()
    }

    /// Returns the list of events that were produced by this WAF run.
    ///
    /// This is only expected to be populated when [`Context::run`] returns [`RunResult::Match`].
    pub fn events(&self) -> Option<&Keyed<WafArray>> {
        debug_assert!(self.data.is_valid());
        self.data
            .get_bstr(b"events")
            .and_then(Keyed::<WafObject>::as_type)
    }

    /// Returns the list of actions that were produced by this WAF run.
    ///
    /// This is only expected to be populated when [`Context::run`] returns [`RunResult::Match`].
    pub fn actions(&self) -> Option<&Keyed<WafMap>> {
        debug_assert!(self.data.is_valid());
        self.data
            .get_bstr(b"actions")
            .and_then(Keyed::<WafObject>::as_type)
    }

    /// Returns the list of attributes that were produced by this WAF run, and which should be
    /// attached to the surrounding trace.
    pub fn attributes(&self) -> Option<&Keyed<WafMap>> {
        debug_assert!(self.data.is_valid());
        self.data
            .get_bstr(b"attributes")
            .and_then(Keyed::<WafObject>::as_type)
    }
}
impl fmt::Debug for RunOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunOutput")
            .field("timeout", &self.timeout())
            .field("keep", &self.keep())
            .field("duration", &self.duration())
            .field("evaluated", &self.evaluated())
            .field("events", &self.events())
            .field("actions", &self.actions())
            .field("attributes", &self.attributes())
            .finish()
    }
}
