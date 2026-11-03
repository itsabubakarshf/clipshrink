//! Small helpers: size parsing/formatting.

use anyhow::{bail, Result};

/// Parse sizes like `50MB`, `50M`, `1.5GB`, `800KB`, `52428800`.
/// Units are decimal (1 MB = 1,000,000 bytes) so a "50 MB" part is also
/// under 50 MiB — safe for uploaders that use either definition.
pub fn parse_size(input: &str) -> Result<u64> {
    let s = input.trim().to_ascii_lowercase().replace(' ', "");
    let split = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let value: f64 = num
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid size '{input}' (try 50MB, 1.5GB, 800KB)"))?;
    let mult: f64 = match unit {
        "" | "b" => 1.0,
        "k" | "kb" => 1e3,
        "m" | "mb" => 1e6,
        "g" | "gb" => 1e9,
        _ => bail!("unknown size unit '{unit}' in '{input}' (use KB, MB or GB)"),
    };
    let bytes = (value * mult) as u64;
    if bytes == 0 {
        bail!("size must be greater than zero");
    }
    Ok(bytes)
}

pub fn fmt_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.2} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.0} KB", b / 1e3)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_units() {
        assert_eq!(parse_size("50MB").unwrap(), 50_000_000);
        assert_eq!(parse_size("50 m").unwrap(), 50_000_000);
        assert_eq!(parse_size("1.5GB").unwrap(), 1_500_000_000);
        assert_eq!(parse_size("800kb").unwrap(), 800_000);
        assert_eq!(parse_size("1234").unwrap(), 1234);
    }

    #[test]
    fn rejects_bad_sizes() {
        assert!(parse_size("abc").is_err());
        assert!(parse_size("5XB").is_err());
        assert!(parse_size("0MB").is_err());
    }

    #[test]
    fn formats() {
        assert_eq!(fmt_size(50_000_000), "50.0 MB");
    }
}
