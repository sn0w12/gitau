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
