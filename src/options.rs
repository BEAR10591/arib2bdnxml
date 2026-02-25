use std::collections::HashMap;

/// Excluded libaribcaption option keys (handled internally or not supported).
const EXCLUDED_OPTS: &[&str] = &["sub_type", "ass_single_rect", "canvas_size"];

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
