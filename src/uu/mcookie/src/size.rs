use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ParseSizeError(String);

impl fmt::Display for ParseSizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid size format: {}", self.0)
    }
}

impl Error for ParseSizeError {}

pub struct Size(u64);

impl Size {
    pub fn parse(s: &str) -> Result<Self, ParseSizeError> {
        let s = s.trim();

        // Handle bytes with "B" suffix
        if s.ends_with('B') && !s.ends_with("iB") {
            if let Some(nums) = s.strip_suffix('B') {
                return nums
                    .trim()
                    .parse::<u64>()
                    .map(Self)
                    .map_err(|_| ParseSizeError(s.to_string()));
            }
        }

        // Handle binary units (KiB, MiB, GiB, TiB)
        for (suffix, exponent) in [("KiB", 1), ("MiB", 2), ("GiB", 3), ("TiB", 4)] {
            if let Some(nums) = s.strip_suffix(suffix) {
                return nums
                    .trim()
                    .parse::<u64>()
                    .map(|n| Self(n * 1024_u64.pow(exponent)))
                    .map_err(|_| ParseSizeError(s.to_string()));
            }
        }

        // If no suffix, treat as bytes
        s.parse::<u64>()
            .map(Self)
            .map_err(|_| ParseSizeError(s.to_string()))
    }

    pub fn size_bytes(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_numeric() {
        assert_eq!(Size::parse("1234").unwrap().size_bytes(), 1234);
    }

    #[test]
    fn test_parse_with_suffix() {
        assert_eq!(Size::parse("1024B").unwrap().size_bytes(), 1024);
        assert_eq!(Size::parse("1KiB").unwrap().size_bytes(), 1024);
        assert_eq!(Size::parse("1MiB").unwrap().size_bytes(), 1024 * 1024);
        assert_eq!(
            Size::parse("1GiB").unwrap().size_bytes(),
            1024 * 1024 * 1024
        );
        assert_eq!(
            Size::parse("1TiB").unwrap().size_bytes(),
            1024 * 1024 * 1024 * 1024
        );
    }

    #[test]
    fn test_invalid_input() {
        // Invalid format
        assert!(Size::parse("invalid").is_err());
    }
}
