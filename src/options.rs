use std::collections::HashMap;

/// Excluded libaribcaption option keys (handled internally or not supported).
const EXCLUDED_OPTS: &[&str] = &["sub_type", "ass_single_rect", "canvas_size"];

/// Parses a time string into seconds.
/// Supports: seconds (123.456), HH:MM:SS, HH:MM:SS.mmm, MM:SS, MM:SS.mmm.
pub fn parse_time_string(time_str: &str) -> Result<f64, String> {
    let s = time_str.trim();
    let colon_count = s.matches(':').count();

    if colon_count == 2 {
        if let Some(dot_pos) = s.find('.') {
            let time_part = &s[..dot_pos];
            let ms_part = format!("0.{}", &s[dot_pos + 1..]);
            let milliseconds: f64 = ms_part.parse().map_err(|_| "invalid milliseconds")?;
            parse_hhmmss(time_part).map(|secs| secs + milliseconds)
        } else {
            parse_hhmmss(s)
        }
    } else if colon_count == 1 {
        if let Some(dot_pos) = s.find('.') {
            let time_part = &s[..dot_pos];
            let ms_part = format!("0.{}", &s[dot_pos + 1..]);
            let milliseconds: f64 = ms_part.parse().map_err(|_| "invalid milliseconds")?;
            parse_mmss(time_part).map(|secs| secs + milliseconds)
        } else {
            parse_mmss(s)
        }
    } else {
        s.parse::<f64>().map_err(|_| {
            "Invalid time format. Use seconds (e.g. 123.456) or HH:MM:SS.mmm (e.g. 01:23:45.123)".to_string()
        })
    }
}

fn parse_hhmmss(s: &str) -> Result<f64, String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err("HH:MM:SS requires 3 numbers".to_string());
    }
    let hours: i32 = parts[0].trim().parse().map_err(|_| "invalid hours")?;
    let minutes: i32 = parts[1].trim().parse().map_err(|_| "invalid minutes")?;
    let seconds: i32 = parts[2].trim().parse().map_err(|_| "invalid seconds")?;
    Ok(hours as f64 * 3600.0 + minutes as f64 * 60.0 + seconds as f64)
}

fn parse_mmss(s: &str) -> Result<f64, String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err("MM:SS requires 2 numbers".to_string());
    }
    let minutes: i32 = parts[0].trim().parse().map_err(|_| "invalid minutes")?;
    let seconds: i32 = parts[1].trim().parse().map_err(|_| "invalid seconds")?;
    Ok(minutes as f64 * 60.0 + seconds as f64)
}

fn is_excluded_opt(key: &str) -> bool {
    EXCLUDED_OPTS.contains(&key)
}

/// Parses libaribcaption option string (key=value,key=value). Values may be quoted; commas inside quotes are not separators.
pub fn parse_libaribcaption_opts(opts_str: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let mut remaining = opts_str.trim();

    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }
        let eq_pos = match remaining.find('=') {
            Some(p) => p,
            None => {
                eprintln!("Warning: libaribcaption option '{}' is not key=value format, skipping", remaining);
                break;
            }
        };
        let key = remaining[..eq_pos].trim().to_string();
        let value_start = eq_pos + 1;
        let mut value = String::new();
        let mut i = value_start;
        let bytes = remaining.as_bytes();
        let len = bytes.len();
        let mut in_quotes = false;
        let mut quote_char = 0u8;

        while i < len {
            let c = bytes[i];
            if (c == b'"' || c == b'\'') && (i == value_start || bytes[i - 1] != b'\\') {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = c;
                } else if c == quote_char {
                    in_quotes = false;
                }
                value.push(c as char);
                i += 1;
                continue;
            }
            if c == b',' && !in_quotes {
                let next_start = (i + 1..len)
                    .find(|&j| bytes[j] != b' ' && bytes[j] != b'\t')
                    .unwrap_or(len);
                let next_eq = remaining[next_start..].find('=').map(|p| next_start + p);
                if let Some(next_eq) = next_eq {
                    let potential_key = remaining[next_start..next_eq].trim();
                    if !potential_key.is_empty() && !potential_key.contains(',') {
                        break;
                    }
                }
            }
            value.push(c as char);
            i += 1;
        }

        let value = value.trim().to_string();
        let value = if value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
        {
            value[1..value.len() - 1].to_string()
        } else {
            value
        };

        if is_excluded_opt(&key) {
            eprintln!("Warning: libaribcaption option '{}' is not supported, skipping", key);
        } else {
            result.insert(key, value);
        }

        if i < len && bytes[i] == b',' {
            remaining = &remaining[i + 1..];
        } else {
            remaining = &remaining[i..];
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_seconds() {
        assert!((parse_time_string("123.456").unwrap() - 123.456).abs() < 1e-9);
        assert!((parse_time_string("0").unwrap()).abs() < 1e-9);
    }

    #[test]
    fn test_parse_time_hhmmss() {
        assert!((parse_time_string("01:23:45").unwrap() - (3600.0 + 23.0 * 60.0 + 45.0)).abs() < 1e-9);
        assert!((parse_time_string("00:00:00").unwrap()).abs() < 1e-9);
    }

    #[test]
    fn test_parse_time_hhmmss_mmm() {
        let v = parse_time_string("01:23:45.123").unwrap();
        assert!((v - (3600.0 + 23.0 * 60.0 + 45.123)).abs() < 1e-6);
    }

    #[test]
    fn test_parse_time_mmss() {
        assert!((parse_time_string("23:45").unwrap() - (23.0 * 60.0 + 45.0)).abs() < 1e-9);
    }

    #[test]
    fn test_parse_libaribcaption_opts() {
        let m = parse_libaribcaption_opts("outline_width=0.0,font=Hiragino");
        assert_eq!(m.get("outline_width"), Some(&"0.0".to_string()));
        assert_eq!(m.get("font"), Some(&"Hiragino".to_string()));
    }

    #[test]
    fn test_parse_libaribcaption_opts_excluded() {
        let m = parse_libaribcaption_opts("sub_type=bitmap,outline_width=0.0");
        assert!(m.get("sub_type").is_none());
        assert_eq!(m.get("outline_width"), Some(&"0.0".to_string()));
    }

    #[test]
    fn test_parse_libaribcaption_opts_quoted() {
        let m = parse_libaribcaption_opts(r#"font="Hiragino Maru Gothic ProN""#);
        assert_eq!(m.get("font"), Some(&"Hiragino Maru Gothic ProN".to_string()));
    }
}
