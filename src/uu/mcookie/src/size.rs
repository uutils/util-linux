use std::num::ParseIntError;

pub struct Size(u64);

impl Size {
    pub fn parse(s: &str) -> Result<Self, ParseIntError> {
        let s = s.trim();

        if s.chars().all(char::is_numeric) {
            return Ok(Self(s.parse::<u64>()?));
        };

        for (suffix, exponent) in [("K", 1), ("M", 2), ("G", 3), ("T", 4)] {
            if let Some(nums) = s.strip_suffix(suffix) {
                let value = nums.trim().parse::<u64>()?;
                let multiplier = 1024_u64.pow(exponent);
                return Ok(Self(value * multiplier));
            }
        }
        Ok(Self(s.parse::<u64>()?))
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
        assert_eq!(Size::parse("1K").unwrap().size_bytes(), 1024);
        assert_eq!(Size::parse("1M").unwrap().size_bytes(), 1024 * 1024);
        assert_eq!(Size::parse("1G").unwrap().size_bytes(), 1024 * 1024 * 1024);
        assert_eq!(
            Size::parse("1T").unwrap().size_bytes(),
            1024 * 1024 * 1024 * 1024
        );
    }

    #[test]
    fn test_invalid_input() {
        assert!(Size::parse("invalid").is_err());
    }
}
