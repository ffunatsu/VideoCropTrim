/// Format seconds into HH:MM:SS.mmm or MM:SS.mmm string.
pub fn format_time(seconds: f64) -> String {
    if seconds.is_nan() || seconds.is_infinite() || seconds < 0.0 {
        return "00:00.000".to_string();
    }

    let total_millis = (seconds * 1000.0).round() as u64;
    let millis = total_millis % 1000;
    let total_secs = total_millis / 1000;
    let secs = total_secs % 60;
    let total_mins = total_secs / 60;
    let mins = total_mins % 60;
    let hours = total_mins / 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis)
    } else {
        format!("{:02}:{:02}.{:03}", mins, secs, millis)
    }
}

/// Format seconds into compact HH:MM:SS string (without millis).
pub fn format_time_compact(seconds: f64) -> String {
    if seconds.is_nan() || seconds.is_infinite() || seconds < 0.0 {
        return "00:00".to_string();
    }

    let total_secs = seconds.round() as u64;
    let secs = total_secs % 60;
    let total_mins = total_secs / 60;
    let mins = total_mins % 60;
    let hours = total_mins / 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{:02}:{:02}", mins, secs)
    }
}

/// Parse time string like "01:23.456" or "01:02:03.456" or "45.2" into seconds.
pub fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Ok(sec) = s.parse::<f64>() {
        return Some(sec);
    }

    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        2 => {
            let mins: f64 = parts[0].parse().ok()?;
            let secs: f64 = parts[1].parse().ok()?;
            Some(mins * 60.0 + secs)
        }
        3 => {
            let hours: f64 = parts[0].parse().ok()?;
            let mins: f64 = parts[1].parse().ok()?;
            let secs: f64 = parts[2].parse().ok()?;
            Some(hours * 3600.0 + mins * 60.0 + secs)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time() {
        assert_eq!(format_time(0.0), "00:00.000");
        assert_eq!(format_time(65.123), "01:05.123");
        assert_eq!(format_time(3665.5), "01:01:05.500");
    }

    #[test]
    fn test_parse_time() {
        assert_eq!(parse_time("10.5"), Some(10.5));
        assert_eq!(parse_time("01:05.500"), Some(65.5));
        assert_eq!(parse_time("01:01:05.500"), Some(3665.5));
    }
}

