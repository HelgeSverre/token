//! Strongly typed byte quantities and binary-unit formatting.

use std::fmt;

const KIBIBYTE: u64 = 1 << 10;
const MEBIBYTE: u64 = 1 << 20;
const GIBIBYTE: u64 = 1 << 30;

/// A byte quantity with const-friendly binary-unit constructors.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteSize(u64);

impl ByteSize {
    pub const fn bytes(value: u64) -> Self {
        Self(value)
    }

    pub const fn kibibytes(value: u64) -> Self {
        Self(Self::checked_scale(value, KIBIBYTE))
    }

    pub const fn mebibytes(value: u64) -> Self {
        Self(Self::checked_scale(value, MEBIBYTE))
    }

    pub const fn gibibytes(value: u64) -> Self {
        Self(Self::checked_scale(value, GIBIBYTE))
    }

    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        assert!(
            self.0 <= usize::MAX as u64,
            "byte size does not fit in usize"
        );
        self.0 as usize
    }

    const fn checked_scale(value: u64, unit: u64) -> u64 {
        match value.checked_mul(unit) {
            Some(bytes) => bytes,
            None => panic!("byte size overflow"),
        }
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (unit, suffix) = if self.0 >= GIBIBYTE {
            (GIBIBYTE, "GiB")
        } else if self.0 >= MEBIBYTE {
            (MEBIBYTE, "MiB")
        } else if self.0 >= KIBIBYTE {
            (KIBIBYTE, "KiB")
        } else {
            return write!(f, "{} B", self.0);
        };
        write!(f, "{:.1} {suffix}", self.0 as f64 / unit as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::ByteSize;

    #[test]
    fn constructors_and_conversions_are_exact() {
        const BYTES: ByteSize = ByteSize::bytes(7);
        const KIB: ByteSize = ByteSize::kibibytes(3);
        const MIB: ByteSize = ByteSize::mebibytes(4);
        const GIB: ByteSize = ByteSize::gibibytes(5);
        const KIB_USIZE: usize = KIB.as_usize();

        assert_eq!(BYTES.as_u64(), 7);
        assert_eq!(KIB.as_u64(), 3_072);
        assert_eq!(MIB.as_u64(), 4_194_304);
        assert_eq!(GIB.as_u64(), 5_368_709_120);
        assert_eq!(KIB_USIZE, 3_072);
    }

    #[test]
    fn byte_sizes_are_ordered_by_byte_count() {
        assert!(ByteSize::kibibytes(1) > ByteSize::bytes(1_023));
        assert_eq!(ByteSize::mebibytes(1), ByteSize::kibibytes(1_024));
    }

    #[test]
    fn display_uses_binary_thresholds_and_fractional_units() {
        assert_eq!(ByteSize::bytes(1_023).to_string(), "1023 B");
        assert_eq!(ByteSize::bytes(1_024).to_string(), "1.0 KiB");
        assert_eq!(ByteSize::bytes(1_536).to_string(), "1.5 KiB");
        assert_eq!(ByteSize::mebibytes(3).to_string(), "3.0 MiB");
        assert_eq!(ByteSize::gibibytes(5).to_string(), "5.0 GiB");
    }

    #[test]
    #[should_panic(expected = "byte size overflow")]
    fn constructors_reject_overflow() {
        let _ = ByteSize::gibibytes(u64::MAX);
    }
}
