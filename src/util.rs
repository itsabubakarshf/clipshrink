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

/// `128k`, `1.5m`, `96000` -> bits per second.
pub fn parse_bitrate(input: &str) -> Result<u64> {
    let s = input.trim().to_ascii_lowercase();
    let (num, mult) = match s.strip_suffix('k') {
        Some(n) => (n, 1e3),
        None => match s.strip_suffix('m') {
            Some(n) => (n, 1e6),
            None => (s.as_str(), 1.0),
        },
    };
    let v: f64 = num.trim().parse().map_err(|_| anyhow::anyhow!("invalid bitrate '{input}' (try 128k)"))?;
    Ok((v * mult) as u64)
}

/// `40`, `40%`, `30-50%` -> fraction of size to remove (a range aims at its middle).
pub fn parse_reduce(input: &str) -> Result<f64> {
    let s = input.trim().replace(['%', ' '], "");
    let nums: Vec<f64> = s
        .split(['-', '–', ' '])
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<f64>().map_err(|_| anyhow::anyhow!("invalid percentage '{input}' (try 40, 40% or 30-50)")))
        .collect::<Result<_>>()?;
    let pct = match nums.as_slice() {
        [a] => *a,
        [a, b] => (a + b) / 2.0,
        _ => bail!("invalid percentage '{input}' (try 40, 40% or 30-50)"),
    };
    if !(1.0..=95.0).contains(&pct) {
        bail!("percentage must be between 1 and 95");
    }
    Ok(pct / 100.0)
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
    fn parses_reduce_and_bitrate() {
        assert_eq!(parse_reduce("40").unwrap(), 0.4);
        assert_eq!(parse_reduce("40%").unwrap(), 0.4);
        assert!((parse_reduce("30-50%").unwrap() - 0.4).abs() < 1e-9);
        assert!(parse_reduce("0").is_err());
        assert!(parse_reduce("99").is_err());
        assert!(parse_reduce("x").is_err());
        assert_eq!(parse_bitrate("128k").unwrap(), 128_000);
        assert_eq!(parse_bitrate("1.5m").unwrap(), 1_500_000);
    }

    #[test]
    fn formats() {
        assert_eq!(fmt_size(50_000_000), "50.0 MB");
    }
}
