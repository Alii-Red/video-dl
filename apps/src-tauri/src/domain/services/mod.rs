//! Pure domain services — rules that span multiple value objects/entities.

use crate::domain::errors::DomainError;
use crate::domain::value_objects::Quality;

/// Pick the highest-ranked quality (resolution first, bitrate as tie-breaker).
pub fn select_best_quality(qualities: &[Quality]) -> Result<&Quality, DomainError> {
    qualities
        .iter()
        .max_by_key(|q| q.rank())
        .ok_or(DomainError::NoQualityAvailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_highest_resolution() {
        let qs = vec![
            Quality::new(Some(480), Some(9000), None, true),
            Quality::new(Some(1080), Some(5000), None, true),
            Quality::new(Some(720), Some(3000), None, true),
        ];
        assert_eq!(select_best_quality(&qs).unwrap().height, Some(1080));
    }

    #[test]
    fn bitrate_breaks_ties() {
        let qs = vec![
            Quality::new(Some(1080), Some(4000), None, true),
            Quality::new(Some(1080), Some(8000), None, true),
        ];
        assert_eq!(select_best_quality(&qs).unwrap().bitrate, Some(8000));
    }

    #[test]
    fn empty_is_error() {
        assert!(select_best_quality(&[]).is_err());
    }
}
