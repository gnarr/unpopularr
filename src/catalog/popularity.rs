use std::collections::BTreeSet;

use chrono::NaiveDate;

/// Days after which an active day's weight halves. A play from last season
/// still counts for something; one from years ago has faded out.
const HALF_LIFE_DAYS: f64 = 180.0;

/// Weighted active days at which the score reaches ~63 (1 − 1/e). About 26.5
/// recent active days round up to 100, so a handful of recent watches already
/// separates "used" from "untouched" without letting heavy users dominate.
const SATURATION_ACTIVE_DAYS: f64 = 5.0;

/// Scores how used an item is, 0–100, from the UTC calendar days on which it
/// was played. Each distinct day weighs `2^(−age_days / HALF_LIFE_DAYS)`, so
/// repeat sessions on one day count once and old activity decays as `today`
/// advances. The weighted sum saturates exponentially toward 100.
///
/// A day after `today` (clock skew between Tautulli and this host) is treated
/// as today rather than weighing more than a fresh play.
pub fn popularity_score(active_days: impl IntoIterator<Item = NaiveDate>, today: NaiveDate) -> u8 {
    let activity: f64 = active_days
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|day| {
            let age_days = (today - day).num_days().max(0) as f64;
            (-age_days / HALF_LIFE_DAYS).exp2()
        })
        .sum();
    // `f64::round` rounds half away from zero; clamping first keeps the cast
    // from ever saturating.
    let score = (100.0 * (1.0 - (-activity / SATURATION_ACTIVE_DAYS).exp())).round();
    score.clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use chrono::{Days, NaiveDate};

    use super::popularity_score;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 13).expect("valid date")
    }

    fn days_ago(days: u64) -> NaiveDate {
        today() - Days::new(days)
    }

    #[test]
    fn never_played_scores_zero() {
        assert_eq!(popularity_score([], today()), 0);
    }

    #[test]
    fn one_active_day_today_scores_eighteen() {
        // 100 · (1 − e^(−1/5)) = 18.13
        assert_eq!(popularity_score([today()], today()), 18);
    }

    #[test]
    fn repeated_sessions_on_one_day_count_once() {
        assert_eq!(
            popularity_score([today(), today(), today()], today()),
            popularity_score([today()], today())
        );
    }

    #[test]
    fn distinct_days_accumulate() {
        // 100 · (1 − e^(−3/5)) = 45.12
        assert_eq!(
            popularity_score([today(), days_ago(1), days_ago(2)], today()),
            45
        );
        // Weights 1 + 0.5 + 0.25: 100 · (1 − e^(−1.75/5)) = 29.53
        assert_eq!(
            popularity_score([today(), days_ago(180), days_ago(360)], today()),
            30
        );
    }

    #[test]
    fn weight_halves_every_half_life() {
        // 100 · (1 − e^(−0.5/5)) = 9.52
        assert_eq!(popularity_score([days_ago(180)], today()), 10);
        // 100 · (1 − e^(−0.25/5)) = 4.88
        assert_eq!(popularity_score([days_ago(360)], today()), 5);
    }

    #[test]
    fn historical_activity_decays_toward_zero() {
        let scores =
            [0, 30, 90, 180, 360, 720, 1095].map(|age| popularity_score([days_ago(age)], today()));
        assert!(scores.windows(2).all(|pair| pair[0] >= pair[1]));
        assert_eq!(scores.last(), Some(&0));
    }

    #[test]
    fn same_history_decays_as_today_advances() {
        let history = [days_ago(10), days_ago(11)];
        let now = popularity_score(history, today());
        let later = popularity_score(history, today() + Days::new(365));
        assert!(later < now);
    }

    #[test]
    fn recent_activity_beats_equivalent_old_activity() {
        assert!(popularity_score([today()], today()) > popularity_score([days_ago(400)], today()));
    }

    #[test]
    fn future_days_weigh_like_today() {
        assert_eq!(
            popularity_score([today() + Days::new(3)], today()),
            popularity_score([today()], today())
        );
    }

    #[test]
    fn scores_stay_within_bounds_and_saturate() {
        let daily_for_a_year = (0..365).map(days_ago);
        assert_eq!(popularity_score(daily_for_a_year, today()), 100);
        for count in 0..60 {
            let score = popularity_score((0..count).map(days_ago), today());
            assert!(score <= 100);
        }
    }
}
