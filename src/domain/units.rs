/// Unit types to distinguish between bytes, megabits (network), and other similar values.
///
/// This prevents accidental confusion between megabytes and megabits, which are both 10^6 but
/// represent different things.
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Div, Mul};

/// A byte count (e.g., cache size, file sizes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Bytes(pub u64);

impl Bytes {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub const ZERO: Self = Self(0);
    /// 1 KiB = 1024 bytes
    pub const ONE_KIB: u64 = 1_024;
    /// 1 MiB = 1024 KiB
    pub const ONE_MIB: u64 = 1_048_576;
    /// 1 GiB = 1024 MiB (binary gigabyte, used for UI display)
    pub const ONE_GIB: u64 = 1_073_741_824;
    /// 1 TiB = 1024 GiB
    pub const ONE_TIB: u64 = 1_099_511_627_776;

    /// Convert to GiB (binary gigabytes) for display
    pub fn as_gib(&self) -> f64 {
        self.0 as f64 / Self::ONE_GIB as f64
    }

    /// Convert to formatted string with abbreviated binary units (KiB, MiB, GiB, TiB).
    pub fn format(&self) -> String {
        if self.0 >= Self::ONE_TIB {
            format!("{:.2} TiB", self.0 as f64 / Self::ONE_TIB as f64)
        } else if self.0 >= Self::ONE_GIB {
            format!("{:.2} GiB", self.0 as f64 / Self::ONE_GIB as f64)
        } else if self.0 >= Self::ONE_MIB {
            format!("{:.2} MiB", self.0 as f64 / Self::ONE_MIB as f64)
        } else if self.0 >= Self::ONE_KIB {
            format!("{:.2} KiB", self.0 as f64 / Self::ONE_KIB as f64)
        } else {
            format!("{} B", self.0)
        }
    }
}

impl fmt::Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl std::ops::Add for Bytes {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
}

impl std::ops::Sub for Bytes {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl Mul<u64> for Bytes {
    type Output = Self;
    fn mul(self, other: u64) -> Self {
        Self(self.0.saturating_mul(other))
    }
}

impl Div<u64> for Bytes {
    type Output = Self;
    fn div(self, other: u64) -> Self {
        Self(self.0 / other)
    }
}

/// A network speed in megabits per second (Mbps).
///
/// IMPORTANT: This is megabits (10^6 bits), not megabytes (10^6 bytes).
/// 1 MB/s = 8 Mbps.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MegabitsPerSecond(pub f64);

impl MegabitsPerSecond {
    pub fn new(value: f64) -> Self {
        Self(value)
    }

    pub fn value(&self) -> f64 {
        self.0
    }

    pub const ZERO: Self = Self(0.0);
    pub const ONE_MBPS: Self = Self(1.0);
    pub const ONE_GBPS: Self = Self(1000.0);

    pub fn is_finite(&self) -> bool {
        self.0.is_finite()
    }

    pub fn is_positive(&self) -> bool {
        self.0 > 0.0
    }

    /// Convert to MB/s (megabytes per second) for reference.
    pub fn as_mbps(&self) -> f64 {
        self.0
    }

    /// Convert to MB/s (megabytes per second).
    pub fn as_mb_per_sec(&self) -> f64 {
        self.0 / 8.0
    }

    pub fn format(&self) -> String {
        if self.0 >= 1000.0 {
            format!("{:.2} Gbps", self.0 / 1000.0)
        } else {
            format!("{:.1} Mbps", self.0)
        }
    }
}

impl fmt::Display for MegabitsPerSecond {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

/// A display name wrapper for validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayName(String);

impl DisplayName {
    pub fn new(name: String) -> Result<Self, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Display name cannot be empty".to_string());
        }
        if trimmed.len() > 128 {
            return Err("Display name cannot exceed 128 characters".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<DisplayName> for String {
    fn from(name: DisplayName) -> Self {
        name.0
    }
}

/// A percentage value (0-100).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Percentage(pub f64);

impl Percentage {
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 100.0))
    }

    pub fn value(&self) -> f64 {
        self.0
    }

    pub fn format(&self) -> String {
        format!("{:.1}%", self.0)
    }
}

impl fmt::Display for Percentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}
