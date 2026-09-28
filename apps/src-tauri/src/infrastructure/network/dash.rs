use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::application::error::ApplicationError;
use crate::domain::value_objects::Quality;

/// What we learn from a DASH manifest (MPD).
#[derive(Debug, Clone, PartialEq)]
pub struct DashInfo {
    pub qualities: Vec<Quality>,
    /// True if any `ContentProtection` element is present (Widevine/PlayReady…).
    /// Protected sources are surfaced but never auto-downloaded.
    pub protected: bool,
}

/// Parse an MPD, collecting `Representation` variants and detecting protection.
/// Event-based (quick-xml) — no full DOM, tolerant of unknown elements.
pub fn analyze_dash(content: &str) -> Result<DashInfo, ApplicationError> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut qualities = Vec::new();
    let mut protected = false;
    // In DASH, mimeType/contentType is often declared on the enclosing
    // AdaptationSet and inherited by its Representations.
    let mut group_audio = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match local_name(&e).as_str() {
                "AdaptationSet" => group_audio = is_audio_group(&e),
                "Representation" => qualities.push(representation(&e, group_audio)),
                "ContentProtection" => protected = true,
                _ => {}
            },
            Ok(Event::Empty(e)) => match local_name(&e).as_str() {
                "Representation" => qualities.push(representation(&e, group_audio)),
                "ContentProtection" => protected = true,
                _ => {}
            },
            Ok(Event::End(e)) => {
                if String::from_utf8_lossy(e.name().local_name().as_ref()) == "AdaptationSet" {
                    group_audio = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ApplicationError::Probe(format!("invalid mpd: {e}"))),
            _ => {}
        }
    }

    Ok(DashInfo {
        qualities,
        protected,
    })
}

fn local_name(e: &BytesStart) -> String {
    String::from_utf8_lossy(e.name().local_name().as_ref()).into_owned()
}

fn attr(e: &BytesStart, name: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        let key = String::from_utf8_lossy(a.key.local_name().as_ref()).into_owned();
        (key == name).then(|| a.unescape_value().map(|v| v.into_owned()).unwrap_or_default())
    })
}

fn is_audio_group(e: &BytesStart) -> bool {
    let mime_audio = attr(e, "mimeType").is_some_and(|m| m.starts_with("audio"));
    let content_audio = attr(e, "contentType").is_some_and(|c| c == "audio");
    mime_audio || content_audio
}

fn representation(e: &BytesStart, group_audio: bool) -> Quality {
    let mime = attr(e, "mimeType");
    let own_audio = mime.as_deref().is_some_and(|m| m.starts_with("audio"));
    Quality {
        height: attr(e, "height").and_then(|v| v.parse::<u32>().ok()),
        bitrate: attr(e, "bandwidth").and_then(|v| v.parse::<u64>().ok()),
        codec: attr(e, "codecs"),
        has_audio: own_audio || group_audio,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MPD: &str = r#"<?xml version="1.0"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="static">
  <Period>
    <AdaptationSet mimeType="video/mp4">
      <Representation id="1" bandwidth="800000" width="640" height="360" codecs="avc1.4d401e"/>
      <Representation id="2" bandwidth="4000000" width="1920" height="1080" codecs="avc1.640028"/>
    </AdaptationSet>
    <AdaptationSet mimeType="audio/mp4">
      <Representation id="a" bandwidth="128000" codecs="mp4a.40.2"/>
    </AdaptationSet>
  </Period>
</MPD>"#;

    const PROTECTED_MPD: &str = r#"<?xml version="1.0"?>
<MPD>
  <Period><AdaptationSet mimeType="video/mp4">
    <ContentProtection schemeIdUri="urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"/>
    <Representation id="1" bandwidth="800000" width="640" height="360"/>
  </AdaptationSet></Period>
</MPD>"#;

    #[test]
    fn collects_representations() {
        let info = analyze_dash(MPD).unwrap();
        assert!(!info.protected);
        assert_eq!(info.qualities.len(), 3);
        let heights: Vec<_> = info.qualities.iter().filter_map(|q| q.height).collect();
        assert!(heights.contains(&360));
        assert!(heights.contains(&1080));
        assert!(info.qualities.iter().any(|q| q.has_audio));
    }

    #[test]
    fn detects_content_protection() {
        assert!(analyze_dash(PROTECTED_MPD).unwrap().protected);
    }
}
