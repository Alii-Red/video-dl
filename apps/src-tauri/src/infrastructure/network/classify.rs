use crate::domain::value_objects::MediaFormat;

/// Classify a URL into a [`MediaFormat`], preferring the HTTP `Content-Type`
/// header and falling back to the URL extension. Returns `None` when it's not a
/// recognized media resource.
pub fn classify(url: &str, content_type: Option<&str>) -> Option<MediaFormat> {
    if let Some(ct) = content_type {
        let ct = ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
        match ct.as_str() {
            "application/vnd.apple.mpegurl"
            | "application/x-mpegurl"
            | "audio/mpegurl"
            | "audio/x-mpegurl" => return Some(MediaFormat::Hls),
            "application/dash+xml" => return Some(MediaFormat::Dash),
            "video/mp4" => return Some(MediaFormat::ProgressiveMp4),
            "video/webm" => return Some(MediaFormat::WebM),
            "video/quicktime" => return Some(MediaFormat::Mov),
            _ => {}
        }
    }

    let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
    if path.ends_with(".m3u8") {
        Some(MediaFormat::Hls)
    } else if path.ends_with(".mpd") {
        Some(MediaFormat::Dash)
    } else if path.ends_with(".mp4") || path.ends_with(".m4v") {
        Some(MediaFormat::ProgressiveMp4)
    } else if path.ends_with(".webm") {
        Some(MediaFormat::WebM)
    } else if path.ends_with(".mov") {
        Some(MediaFormat::Mov)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_wins() {
        assert_eq!(
            classify("https://x/stream", Some("application/vnd.apple.mpegurl")),
            Some(MediaFormat::Hls)
        );
        assert_eq!(
            classify("https://x/m", Some("application/dash+xml; charset=utf-8")),
            Some(MediaFormat::Dash)
        );
        assert_eq!(
            classify("https://x/f", Some("video/mp4")),
            Some(MediaFormat::ProgressiveMp4)
        );
    }

    #[test]
    fn extension_fallback_ignores_query() {
        assert_eq!(
            classify("https://x/v.m3u8?token=abc", None),
            Some(MediaFormat::Hls)
        );
        assert_eq!(classify("https://x/v.mpd", None), Some(MediaFormat::Dash));
        assert_eq!(
            classify("https://x/v.mp4#t=10", None),
            Some(MediaFormat::ProgressiveMp4)
        );
        assert_eq!(classify("https://x/v.webm", None), Some(MediaFormat::WebM));
        assert_eq!(classify("https://x/v.mov", None), Some(MediaFormat::Mov));
    }

    #[test]
    fn unknown_returns_none() {
        assert_eq!(classify("https://x/page.html", Some("text/html")), None);
        assert_eq!(classify("https://x/whatever", None), None);
    }
}
