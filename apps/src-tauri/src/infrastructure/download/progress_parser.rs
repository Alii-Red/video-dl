use crate::domain::value_objects::Progress;

/// yt-dlp `--progress-template` we pass so stdout carries a stable, parseable
/// line. Tab-separated: prefix, percent string, speed (bytes/s or NA), eta
/// (seconds or NA). Combined with `--newline` and `--color never`.
pub const PROGRESS_TEMPLATE: &str =
    "VDLP\t%(progress._percent_str)s\t%(progress.speed)s\t%(progress.eta)s";

const PREFIX: &str = "VDLP\t";

/// Parse one stdout line into [`Progress`]. Returns `None` for non-progress
/// lines (yt-dlp prints plenty of those).
pub fn parse_progress_line(line: &str) -> Option<Progress> {
    let rest = line.trim().strip_prefix(PREFIX)?;
    let mut parts = rest.split('\t');

    let percent = parse_percent(parts.next()?)?;
    let speed_bps = parts.next().and_then(parse_optional_number);
    let eta_secs = parts.next().and_then(parse_optional_number);

    Some(Progress {
        percent,
        speed_bps,
        eta_secs,
    })
}

fn parse_percent(s: &str) -> Option<f32> {
    let s = s.trim().trim_end_matches('%').trim();
    s.parse::<f32>().ok().map(|p| p.clamp(0.0, 100.0))
}

/// yt-dlp emits `NA` (or sometimes `None`) when a value is unknown. Numbers may
/// be floats (speed). Returns `None` for unknown/absent values.
fn parse_optional_number(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("na") || s.eq_ignore_ascii_case("none") {
        return None;
    }
    s.parse::<f64>().ok().map(|v| v.round().max(0.0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_line() {
        let p = parse_progress_line("VDLP\t 42.5%\t1048576.0\t37").unwrap();
        assert!((p.percent - 42.5).abs() < 0.01);
        assert_eq!(p.speed_bps, Some(1_048_576));
        assert_eq!(p.eta_secs, Some(37));
    }

    #[test]
    fn handles_na_values() {
        let p = parse_progress_line("VDLP\t100%\tNA\tNA").unwrap();
        assert_eq!(p.percent, 100.0);
        assert_eq!(p.speed_bps, None);
        assert_eq!(p.eta_secs, None);
    }

    #[test]
    fn ignores_non_progress_lines() {
        assert!(parse_progress_line("[download] Destination: video.mp4").is_none());
        assert!(parse_progress_line("").is_none());
        assert!(parse_progress_line("VDLPX garbage").is_none());
    }

    #[test]
    fn clamps_percent() {
        let p = parse_progress_line("VDLP\t150%\tNA\tNA").unwrap();
        assert_eq!(p.percent, 100.0);
    }

    #[test]
    fn tolerates_trailing_whitespace_and_cr() {
        let p = parse_progress_line("VDLP\t50.0%\t2000.9\t10\r").unwrap();
        assert_eq!(p.speed_bps, Some(2001));
        assert_eq!(p.eta_secs, Some(10));
    }
}
