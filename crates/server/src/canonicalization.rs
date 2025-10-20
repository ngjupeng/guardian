use std::time::Duration;

/// Configuration for delta canonicalization behavior
#[derive(Clone, Debug)]
pub struct CanonicalizationConfig {
    /// Time to wait before checking on-chain commitment (in seconds)
    pub delay_seconds: u64,

    /// How often the worker checks for deltas to canonicalize (in seconds)
    pub check_interval_seconds: u64,
}

impl Default for CanonicalizationConfig {
    fn default() -> Self {
        Self {
            delay_seconds: 15 * 60,     // 15 minutes
            check_interval_seconds: 60, // 1 minute
        }
    }
}

impl CanonicalizationConfig {
    /// Create a new canonicalization config with custom settings
    pub fn new(delay_seconds: u64, check_interval_seconds: u64) -> Self {
        Self {
            delay_seconds,
            check_interval_seconds,
        }
    }

    /// Get delay as Duration
    pub fn delay(&self) -> Duration {
        Duration::from_secs(self.delay_seconds)
    }

    /// Get check interval as Duration
    pub fn check_interval(&self) -> Duration {
        Duration::from_secs(self.check_interval_seconds)
    }
}

/// Mode for handling delta submissions
#[derive(Clone, Debug)]
pub enum CanonicalizationMode {
    /// Run canonicalization worker with on-chain verification
    Enabled(CanonicalizationConfig),

    /// Optimistically accept deltas as valid without on-chain verification
    /// State updates happen immediately on push
    Optimistic,
}

impl Default for CanonicalizationMode {
    fn default() -> Self {
        Self::Enabled(CanonicalizationConfig::default())
    }
}
