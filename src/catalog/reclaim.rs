use chrono::{DateTime, NaiveDate, Utc};

use super::PlaybackMetrics;

/// Days after which both protections halve: an item last played this long ago
/// is half idle, and one added this long ago is half mature.
const HALF_LIFE_DAYS: f64 = 180.0;

/// Size at which disk value reaches half. Value saturates hyperbolically, so a
/// 500 GiB item counts only slightly more than a 50 GiB one, while files much
/// smaller than this barely register however old and unused they are.
const SIZE_SATURATION_GIB: f64 = 5.0;

const BYTES_PER_GIB: f64 = (1u64 << 30) as f64;

/// Scores how attractive an item is to delete to reclaim disk space, 0–100.
/// The product of unpopularity, idle time, maturity, and disk value, so every
/// factor has to agree for the score to get high: anything played today or
/// added today scores 0.
///
/// `None` rather than a guess when an input is unknown: playback unavailable
/// (`popularity_score` or `playback` is `None`), no `added_at`, or plays
/// without a `last_played_at`. Timestamps after `today` (clock skew) count as
/// today.
pub fn reclaim_score(
    popularity_score: Option<u8>,
    playback: Option<&PlaybackMetrics>,
    added_at: Option<DateTime<Utc>>,
    size_on_disk_bytes: i64,
    today: NaiveDate,
) -> Option<u8> {
    let unpopularity = 1.0 - f64::from(popularity_score?) / 100.0;
    let playback = playback?;
    let idle = if playback.play_count == 0 {
        1.0
    } else {
        half_life_progress(days_before(today, playback.last_played_at?))
    };
    let maturity = half_life_progress(days_before(today, added_at?));
    let size_gib = size_on_disk_bytes.max(0) as f64 / BYTES_PER_GIB;
    let space_value = size_gib / (size_gib + SIZE_SATURATION_GIB);
    // Clamping before the cast keeps it from saturating, as in popularity.
    let score = (100.0 * unpopularity * idle * maturity * space_value).round();
    Some(score.clamp(0.0, 100.0) as u8)
}

/// `1 − 2^(−days / HALF_LIFE_DAYS)`: 0 today, 0.5 after one half-life, → 1.
fn half_life_progress(days: f64) -> f64 {
    1.0 - (-days / HALF_LIFE_DAYS).exp2()
}

/// Whole UTC calendar days from `at` to `today`, never negative.
fn days_before(today: NaiveDate, at: DateTime<Utc>) -> f64 {
    (today - at.date_naive()).num_days().max(0) as f64
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Days, NaiveDate, Utc};

    use super::{PlaybackMetrics, reclaim_score};

    const GIB: i64 = 1 << 30;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 13).expect("valid date")
    }

    /// Mid-afternoon UTC, so only the calendar day matters.
    fn days_ago(days: i64) -> DateTime<Utc> {
        let day = if days >= 0 {
            today() - Days::new(days.unsigned_abs())
        } else {
            today() + Days::new(days.unsigned_abs())
        };
        day.and_hms_opt(15, 30, 0).expect("valid time").and_utc()
    }

    fn never_played() -> PlaybackMetrics {
        PlaybackMetrics {
            play_count: 0,
            play_duration_seconds: 0,
            last_played_at: None,
        }
    }

    fn last_played(days: i64) -> PlaybackMetrics {
        PlaybackMetrics {
            play_count: 1,
            play_duration_seconds: 3600,
            last_played_at: Some(days_ago(days)),
        }
    }

    /// Never-played, popularity 0: only maturity and size vary.
    fn unused(age_days: i64, size_bytes: i64) -> Option<u8> {
        reclaim_score(
            Some(0),
            Some(&never_played()),
            Some(days_ago(age_days)),
            size_bytes,
            today(),
        )
    }

    /// Played once `idle_days` ago, two years in the library, 50 GiB.
    fn played(popularity: u8, idle_days: i64) -> Option<u8> {
        reclaim_score(
            Some(popularity),
            Some(&last_played(idle_days)),
            Some(days_ago(730)),
            50 * GIB,
            today(),
        )
    }

    #[test]
    fn unavailable_playback_is_unknown() {
        let added = Some(days_ago(730));
        assert_eq!(reclaim_score(None, None, added, 50 * GIB, today()), None);
        assert_eq!(
            reclaim_score(None, Some(&never_played()), added, 50 * GIB, today()),
            None
        );
        assert_eq!(reclaim_score(Some(0), None, added, 50 * GIB, today()), None);
    }

    #[test]
    fn unknown_added_at_is_unknown() {
        assert_eq!(
            reclaim_score(Some(0), Some(&never_played()), None, 50 * GIB, today()),
            None
        );
    }

    #[test]
    fn plays_without_a_last_played_date_are_unknown() {
        let undated = PlaybackMetrics {
            last_played_at: None,
            ..last_played(0)
        };
        assert_eq!(
            reclaim_score(
                Some(0),
                Some(&undated),
                Some(days_ago(730)),
                50 * GIB,
                today()
            ),
            None
        );
    }

    #[test]
    fn never_played_counts_as_fully_idle() {
        // 100 · (1 − 2^(−730/180)) · 50/55 = 85.44
        assert_eq!(unused(730, 50 * GIB), Some(85));
        // A stray last-played time does not override a zero play count.
        let stray = PlaybackMetrics {
            last_played_at: Some(days_ago(0)),
            ..never_played()
        };
        assert_eq!(
            reclaim_score(
                Some(0),
                Some(&stray),
                Some(days_ago(730)),
                50 * GIB,
                today()
            ),
            Some(85)
        );
    }

    #[test]
    fn played_today_scores_zero_at_any_size() {
        for size in [GIB, 50 * GIB, 1024 * GIB, i64::MAX] {
            let score = reclaim_score(
                Some(0),
                Some(&last_played(0)),
                Some(days_ago(3650)),
                size,
                today(),
            );
            assert_eq!(score, Some(0));
        }
    }

    #[test]
    fn future_last_played_counts_as_today() {
        assert_eq!(played(0, -3), played(0, 0));
        assert_eq!(played(0, -3), Some(0));
    }

    #[test]
    fn future_added_at_counts_as_added_today() {
        assert_eq!(unused(-3, 50 * GIB), unused(0, 50 * GIB));
        assert_eq!(unused(-3, 50 * GIB), Some(0));
    }

    #[test]
    fn newly_added_media_scores_low() {
        assert_eq!(unused(0, 50 * GIB), Some(0));
        // 100 · (1 − 2^(−7/180)) · 50/55 = 2.42
        assert_eq!(unused(7, 50 * GIB), Some(2));
        // 100 · (1 − 2^(−30/180)) · 50/55 = 9.92
        assert_eq!(unused(30, 50 * GIB), Some(10));
    }

    #[test]
    fn maturity_rises_with_age() {
        let scores = [0, 7, 30, 90, 180, 365, 730, 1825].map(|age| unused(age, 50 * GIB));
        assert!(scores.windows(2).all(|pair| pair[0] < pair[1]));
        // One half-life: 100 · 0.5 · 50/55 = 45.45
        assert_eq!(unused(180, 50 * GIB), Some(45));
    }

    #[test]
    fn idle_rises_with_time_since_last_play() {
        let scores = [0, 7, 30, 90, 180, 365, 730].map(|days| played(0, days));
        assert!(scores.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn larger_media_scores_higher_with_diminishing_returns() {
        let scores = [1, 5, 50, 500].map(|gib| unused(730, gib * GIB).expect("known"));
        assert!(scores.windows(2).all(|pair| pair[0] < pair[1]));
        // Ten times the size is nowhere near ten times the score.
        // 100 · (1 − 2^(−730/180)) · 500/505 = 93.06
        assert_eq!(scores, [16, 47, 85, 93]);
    }

    #[test]
    fn tiny_media_never_becomes_a_top_candidate() {
        // 100 · (1 − 2^(−1825/180)) · (100/1024) / (100/1024 + 5) = 1.91
        assert_eq!(unused(1825, 100 * (1 << 20)), Some(2));
        assert_eq!(unused(1825, 0), Some(0));
        assert_eq!(unused(1825, -1), Some(0));
    }

    #[test]
    fn popularity_lowers_the_score() {
        let scores = [0, 10, 25, 50, 75, 100].map(|popularity| played(popularity, 365));
        assert!(scores.windows(2).all(|pair| pair[0] > pair[1]));
        assert_eq!(played(100, 365), Some(0));
    }

    #[test]
    fn representative_values() {
        // 100 · 0.95 · (1 − 2^(−365/180)) · (1 − 2^(−1095/180)) · 40/45 = 62.80
        assert_eq!(
            reclaim_score(
                Some(5),
                Some(&last_played(365)),
                Some(days_ago(1095)),
                40 * GIB,
                today()
            ),
            Some(63)
        );
        // 100 · 0.90 · 0.5 · (1 − 2^(−730/180)) · 20/25 = 33.84
        assert_eq!(
            reclaim_score(
                Some(10),
                Some(&last_played(180)),
                Some(days_ago(730)),
                20 * GIB,
                today()
            ),
            Some(34)
        );
        // 100 · 0.85 · (1 − 2^(−30/180)) · (1 − 2^(−730/180)) · 50/55 = 7.92
        assert_eq!(played(15, 30), Some(8));
    }

    #[test]
    fn scores_stay_within_bounds() {
        for age in [0, 1, 180, 3650, 36500] {
            for size in [0, GIB, 1024 * GIB, i64::MAX] {
                let score = unused(age, size).expect("known");
                assert!(score <= 100);
            }
        }
        assert_eq!(unused(36500, i64::MAX), Some(100));
    }
}
