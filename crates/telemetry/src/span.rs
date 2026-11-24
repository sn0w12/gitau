#[cfg(feature = "enabled")]
use crate::imp;
use crate::AttrValue;

/// Times the enclosing scope. Dropping the guard ends the span and, when it is
/// the outermost span, submits the finished trace.
///
/// Outside a [`crate::scope`] or [`crate::sync_scope`], entering a span is a
/// no-op. With the `enabled` feature off, the guard is zero-sized.
#[must_use = "the span ends when the guard is dropped"]
pub struct SpanGuard {
    #[cfg(feature = "enabled")]
    inner: Option<imp::Guard>,
}

/// A scope captured on one thread so blocking work on another thread can join
/// the same trace. Create with [`ScopeHandle::capture`], resume with
/// [`attach_scope`].
#[derive(Clone)]
pub struct ScopeHandle {
    #[cfg(feature = "enabled")]
    inner: Option<imp::ScopeHandle>,
}

impl ScopeHandle {
    /// Captures the active scope. Returns `None` outside a [`crate::scope`] or
    /// [`crate::sync_scope`], and when collection is off.
    #[inline]
    pub fn capture() -> Option<Self> {
        #[cfg(feature = "enabled")]
        {
            imp::capture_scope().map(|inner| Self { inner: Some(inner) })
        }
        #[cfg(not(feature = "enabled"))]
        {
            None
        }
    }
}

/// Runs blocking code with a captured scope installed on the current thread,
/// so spans it opens join the parent trace instead of starting a new one.
///
/// Attaching does not sample and does not open a trace. Pass `None` to run `f`
/// unchanged; that is what [`ScopeHandle::capture`] returns when collection is
/// off.
pub fn attach_scope<R>(handle: Option<ScopeHandle>, f: impl FnOnce() -> R) -> R {
    #[cfg(feature = "enabled")]
    {
        match handle.and_then(|handle| handle.inner) {
            Some(inner) => imp::attach_scope(&inner, f),
            None => f(),
        }
    }
    #[cfg(not(feature = "enabled"))]
    {
        let _ = handle;
        f()
    }
}

impl SpanGuard {
    #[inline]
    pub fn enter(name: &'static str) -> Self {
        #[cfg(feature = "enabled")]
        {
            Self {
                inner: imp::enter(name),
            }
        }
        #[cfg(not(feature = "enabled"))]
        {
            let _ = name;
            Self {}
        }
    }

    /// Opens the function's own span. Used by [`crate::trace`]. When the
    /// caller already opened a call-site span with the same name, this is a
    /// no-op so annotated functions do not produce duplicate spans.
    #[doc(hidden)]
    #[inline]
    pub fn enter_function(name: &'static str) -> Self {
        #[cfg(feature = "enabled")]
        {
            Self {
                inner: imp::enter_function(name),
            }
        }
        #[cfg(not(feature = "enabled"))]
        {
            let _ = name;
            Self {}
        }
    }

    /// Attaches a value to this span specifically. [`crate::record_attribute`]
    /// targets the innermost active span instead.
    pub fn set_attribute(&self, key: &'static str, value: AttrValue) {
        #[cfg(feature = "enabled")]
        {
            if let Some(inner) = &self.inner {
                inner.set_attribute(key, value);
            }
        }
        #[cfg(not(feature = "enabled"))]
        {
            let _ = (key, value);
        }
    }
}
