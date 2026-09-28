use crate::application::error::ApplicationError;
use crate::domain::value_objects::Quality;

/// What we learn from an HLS playlist.
#[derive(Debug, Clone, PartialEq)]
pub struct HlsInfo {
    /// Quality variants (only present for a master playlist).
    pub qualities: Vec<Quality>,
    /// True if the playlist declares encryption (EXT-X-KEY with METHOD != NONE).
    pub protected: bool,
    pub is_master: bool,
}

/// Parse an M3U8 playlist. Quality variants come from a master playlist;
/// encryption is detected by scanning the key tags (robust across m3u8-rs
/// versions, and works even for media playlists).
pub fn analyze_hls(content: &str) -> Result<HlsInfo, ApplicationError> {
    let protected = detect_encryption(content);

    match m3u8_rs::parse_playlist_res(content.as_bytes()) {
        Ok(m3u8_rs::Playlist::MasterPlaylist(master)) => {
            let qualities = master
                .variants
                .iter()
                .map(|v| Quality {
                    height: v.resolution.as_ref().map(|r| r.height as u32),
                    bitrate: Some(v.bandwidth),
                    codec: v.codecs.clone(),
                    // Master variants are conventionally muxed audio+video.
                    has_audio: true,
                })
                .collect();
            Ok(HlsInfo {
                qualities,
                protected,
                is_master: true,
            })
        }
        Ok(m3u8_rs::Playlist::MediaPlaylist(_)) => Ok(HlsInfo {
            qualities: Vec::new(),
            protected,
            is_master: false,
        }),
        Err(e) => Err(ApplicationError::Probe(format!("invalid m3u8: {e:?}"))),
    }
}

fn detect_encryption(content: &str) -> bool {
    content
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("#EXT-X-KEY") || l.starts_with("#EXT-X-SESSION-KEY"))
        .any(|l| match l.find("METHOD=") {
            Some(idx) => {
                let method = l[idx + "METHOD=".len()..]
                    .split([',', ' '])
                    .next()
                    .unwrap_or("");
                !method.eq_ignore_ascii_case("NONE")
            }
            None => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASTER: &str = "#EXTM3U\n\
#EXT-X-STREAM-INF:BANDWIDTH=1280000,RESOLUTION=640x360,CODECS=\"avc1.42e01e\"\n\
low.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080,CODECS=\"avc1.640028\"\n\
high.m3u8\n";

    const ENCRYPTED_MEDIA: &str = "#EXTM3U\n\
#EXT-X-VERSION:3\n\
#EXT-X-KEY:METHOD=AES-128,URI=\"https://x/key\"\n\
#EXTINF:9.0,\n\
seg0.ts\n\
#EXT-X-ENDLIST\n";

    #[test]
    fn master_yields_variants() {
        let info = analyze_hls(MASTER).unwrap();
        assert!(info.is_master);
        assert!(!info.protected);
        assert_eq!(info.qualities.len(), 2);
        let heights: Vec<_> = info.qualities.iter().filter_map(|q| q.height).collect();
        assert!(heights.contains(&360));
        assert!(heights.contains(&1080));
    }

    #[test]
    fn detects_aes_encryption() {
        let info = analyze_hls(ENCRYPTED_MEDIA).unwrap();
        assert!(!info.is_master);
        assert!(info.protected);
    }

    #[test]
    fn method_none_is_not_protected() {
        let content = "#EXTM3U\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:9.0,\nseg0.ts\n";
        assert!(!analyze_hls(content).unwrap().protected);
    }

    #[test]
    fn invalid_playlist_errors() {
        assert!(analyze_hls("not a playlist at all").is_err());
    }
}
