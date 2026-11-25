use windows::Win32::System::SystemInformation::GetLocalTime;

/// Parses an `"HH:MM"` string into minutes since midnight (`0..1440`).
///
/// Rejects invalid hours (`>= 24`), invalid minutes (`>= 60`), and malformed strings.
pub fn parse_hhmm(s: &str) -> Option<u16> {
    let (hour_str, min_str) = s.trim().split_once(':')?;
    if !(1..=2).contains(&hour_str.len())
        || min_str.len() != 2
        || !hour_str.bytes().all(|b| b.is_ascii_digit())
        || !min_str.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }

    let hours: u16 = hour_str.parse().ok()?;
    let mins: u16 = min_str.parse().ok()?;
    if hours >= 24 || mins >= 60 {
        return None;
    }

    Some(hours * 60 + mins)
}

/// Returns whether `current_mins` falls inside the half-open `[start, end)` window.
///
/// Supports both daytime windows (e.g. `08:00`..`17:00`) and overnight wrap-around
/// windows (e.g. `23:00`..`07:00`). Returns `false` when `start == end` or inputs are invalid.
pub fn is_in_night_window(start: &str, end: &str, current_mins: u16) -> bool {
    if start == end || current_mins >= 1440 {
        return false;
    }

    let (Some(start_mins), Some(end_mins)) = (parse_hhmm(start), parse_hhmm(end)) else {
        return false;
    };

    if start_mins == end_mins {
        return false;
    }

    if start_mins < end_mins {
        current_mins >= start_mins && current_mins < end_mins
    } else {
        current_mins >= start_mins || current_mins < end_mins
    }
}

/// Returns the current local time in minutes since midnight (`0..1440`).
pub fn current_local_minutes() -> u16 {
    let st = unsafe { GetLocalTime() };
    st.wHour * 60 + st.wMinute
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hhmm_valid_and_edge_cases() {
        assert_eq!(parse_hhmm("00:00"), Some(0));
        assert_eq!(parse_hhmm("07:05"), Some(7 * 60 + 5));
        assert_eq!(parse_hhmm("9:30"), Some(9 * 60 + 30));
        assert_eq!(parse_hhmm("23:59"), Some(23 * 60 + 59));
        assert_eq!(parse_hhmm(" 08:15 "), Some(8 * 60 + 15));

        assert_eq!(parse_hhmm(""), None);
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("23:60"), None);
        assert_eq!(parse_hhmm("12:5"), None);
        assert_eq!(parse_hhmm("008:00"), None);
        assert_eq!(parse_hhmm("12:30:00"), None);
        assert_eq!(parse_hhmm("-1:00"), None);
        assert_eq!(parse_hhmm("ab:cd"), None);
    }

    #[test]
    fn daytime_window() {
        assert!(!is_in_night_window("08:00", "17:00", 7 * 60 + 59));
        assert!(is_in_night_window("08:00", "17:00", 8 * 60));
        assert!(is_in_night_window("08:00", "17:00", 12 * 60));
        assert!(is_in_night_window("08:00", "17:00", 16 * 60 + 59));
        assert!(!is_in_night_window("08:00", "17:00", 17 * 60));
    }

    #[test]
    fn overnight_wrap_around_window() {
        assert!(!is_in_night_window("23:00", "07:00", 22 * 60 + 59));
        assert!(is_in_night_window("23:00", "07:00", 23 * 60));
        assert!(is_in_night_window("23:00", "07:00", 23 * 60 + 30));
        assert!(is_in_night_window("23:00", "07:00", 0));
        assert!(is_in_night_window("23:00", "07:00", 2 * 60));
        assert!(is_in_night_window("23:00", "07:00", 6 * 60 + 59));
        assert!(!is_in_night_window("23:00", "07:00", 7 * 60));
        assert!(!is_in_night_window("23:00", "07:00", 12 * 60));
    }

    #[test]
    fn equal_start_and_end_and_invalid_inputs() {
        assert!(!is_in_night_window("08:00", "08:00", 8 * 60));
        assert!(!is_in_night_window("23:00", "23:00", 23 * 60));
        assert!(!is_in_night_window("8:00", "08:00", 8 * 60));
        assert!(!is_in_night_window("invalid", "07:00", 0));
        assert!(!is_in_night_window("23:00", "07:00", 1440));
    }
}
