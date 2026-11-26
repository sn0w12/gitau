use serde::{Deserialize, Serialize};

/// Coarse hardware class so traces can be grouped without identifying a
/// machine. Buckets overlap enough that adjacent machines are indistinguishable
/// in reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuTier {
    Minimal,
    Low,
    Medium,
    High,
    VeryHigh,
}

impl CpuTier {
    pub fn from_logical_cores(cores: usize) -> Self {
        match cores {
            0..=2 => Self::Minimal,
            3..=4 => Self::Low,
            5..=8 => Self::Medium,
            9..=16 => Self::High,
            _ => Self::VeryHigh,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsFamily {
    Windows,
    #[serde(rename = "macos")]
    MacOs,
    Linux,
    Other,
}

impl OsFamily {
    fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    X86,
    X86_64,
    Aarch64,
    Other,
}

impl Arch {
    fn current() -> Self {
        if cfg!(target_arch = "x86_64") {
            Self::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Self::Aarch64
        } else if cfg!(target_arch = "x86") {
            Self::X86
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    fn current() -> Self {
        if cfg!(debug_assertions) {
            Self::Debug
        } else {
            Self::Release
        }
    }
}

/// Context attached to every batch. Contains no identifier for the machine,
/// install, or user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentContext {
    pub app_version: String,
    pub build_profile: BuildProfile,
    pub os: OsFamily,
    pub arch: Arch,
    pub cpu_tier: CpuTier,
}

impl EnvironmentContext {
    /// Reads only the target triple and the logical core count.
    pub fn detect(app_version: impl Into<String>) -> Self {
        let cores = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1);
        Self {
            app_version: app_version.into(),
            build_profile: BuildProfile::current(),
            os: OsFamily::current(),
            arch: Arch::current(),
            cpu_tier: CpuTier::from_logical_cores(cores),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) context: EnvironmentContext,
    pub(crate) enabled: bool,
    pub(crate) sample_permille: u16,
    pub(crate) buffer_capacity: usize,
    pub(crate) max_spans_per_trace: usize,
    pub(crate) max_depth: usize,
    pub(crate) max_attributes_per_span: usize,
    pub(crate) min_trace_duration_us: u64,
    pub(crate) env_kill_switch: bool,
}

impl Config {
    /// Collection starts disabled. This is the initial state at [`crate::init`]
    /// unless [`crate::set_enabled`] was called before it.
    pub fn new(context: EnvironmentContext) -> Self {
        Self {
            context,
            enabled: false,
            sample_permille: 250,
            buffer_capacity: 64,
            max_spans_per_trace: 512,
            max_depth: 32,
            max_attributes_per_span: 16,
            min_trace_duration_us: 0,
            env_kill_switch: true,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn context(&self) -> &EnvironmentContext {
        &self.context
    }

    /// Fraction of task scopes that are traced, in parts per thousand. `1000`
    /// traces everything.
    pub fn sample_permille(mut self, permille: u16) -> Self {
        self.sample_permille = permille.min(1000);
        self
    }

    /// Maximum completed traces held between flushes. The oldest is dropped
    /// when the buffer is full.
    pub fn buffer_capacity(mut self, traces: usize) -> Self {
        self.buffer_capacity = traces.max(1);
        self
    }

    pub fn max_spans_per_trace(mut self, spans: usize) -> Self {
        self.max_spans_per_trace = spans.max(1);
        self
    }

    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth.max(1);
        self
    }

    pub fn max_attributes_per_span(mut self, attributes: usize) -> Self {
        self.max_attributes_per_span = attributes;
        self
    }

    /// Traces shorter than this are discarded at submission.
    pub fn min_trace_duration_us(mut self, micros: u64) -> Self {
        self.min_trace_duration_us = micros;
        self
    }

    /// Whether `TELEMETRY_ENABLED=false` (and equivalents), or `DO_NOT_TRACK=1`
    /// (and equivalents), can override the configuration. Defaults to on.
    pub fn env_kill_switch(mut self, enabled: bool) -> Self {
        self.env_kill_switch = enabled;
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("telemetry is already initialized")]
    AlreadyInitialized,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_core_counts() {
        assert_eq!(CpuTier::from_logical_cores(1), CpuTier::Minimal);
        assert_eq!(CpuTier::from_logical_cores(2), CpuTier::Minimal);
        assert_eq!(CpuTier::from_logical_cores(4), CpuTier::Low);
        assert_eq!(CpuTier::from_logical_cores(8), CpuTier::Medium);
        assert_eq!(CpuTier::from_logical_cores(16), CpuTier::High);
        assert_eq!(CpuTier::from_logical_cores(64), CpuTier::VeryHigh);
    }

    #[test]
    fn config_clamps_values() {
        let config = Config::new(EnvironmentContext::detect("test"))
            .sample_permille(4000)
            .buffer_capacity(0)
            .max_depth(0);
        assert_eq!(config.sample_permille, 1000);
        assert_eq!(config.buffer_capacity, 1);
        assert_eq!(config.max_depth, 1);
        assert!(!config.enabled);
    }

    #[test]
    fn detects_without_identifiers() {
        let context = EnvironmentContext::detect("1.2.3");
        assert_eq!(context.app_version, "1.2.3");
    }
}
