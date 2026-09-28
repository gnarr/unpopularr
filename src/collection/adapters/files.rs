//! Optional filesystem probe.
//!
//! The *arr apps report when *they* learned about a file, which is not when the
//! file arrived: rebuild a Sonarr database and every series claims to be days
//! old. When `[media_probe]` is configured this probe stats the files those apps
//! point at and folds their timestamps into the same acquisition date, so the
//! oldest evidence still wins.
//!
//! Nothing here can fail a sync. Every unreadable path simply contributes no
//! hint, and the counters it keeps exist to make a wrong path mapping visible in
//! the log rather than silently useless.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Utc};
use futures::{StreamExt, stream};
use tokio::{task, time};
use tracing::{debug, info, warn};

use crate::{
    collection::{Snapshot, earliest},
    config::{MediaProbeConfig, PathMapping},
};

/// Matches the bound on concurrent *arr requests: enough to hide per-call
/// latency on network storage without swamping the blocking pool.
const MAX_CONCURRENT_STATS: usize = 8;

/// A hard-mounted NFS stat is uninterruptible, so a dead mount would otherwise
/// park the sync in `running` until the process restarts. The abandoned
/// blocking threads run to completion; a visible timeout is the better trade.
const PROBE_TIMEOUT: Duration = Duration::from_secs(120);

/// 1990-01-01T00:00:00Z. Broken SMB mounts and unpacked archives hand out
/// epoch-0 and year-1601 timestamps; because the fold takes a minimum, one of
/// those would age an item forever.
const EARLIEST_PLAUSIBLE: i64 = 631_152_000;

/// Tolerates clock skew between this host and the storage, and files given a
/// deliberately future mtime.
const FUTURE_TOLERANCE: Duration = Duration::from_secs(24 * 60 * 60);

pub struct FileProbe {
    mappings: Vec<PathMapping>,
}

impl FileProbe {
    /// Warns about mappings that point nowhere now, rather than leaving the
    /// discovery to the first sync six hours later. A missing target is not
    /// fatal: a network mount may still be settling.
    pub fn new(config: MediaProbeConfig) -> Self {
        for mapping in &config.path_mappings {
            if !mapping.to.is_dir() {
                warn!(
                    directory = %mapping.to.display(),
                    "media probe: mapped directory does not exist yet; media ages will fall back to the *arr dates"
                );
            }
        }
        Self {
            mappings: config.path_mappings,
        }
    }

    /// Lowers the snapshot's acquisition dates to the oldest timestamp found on
    /// disk. Returns nothing: there is no failure a caller could act on.
    pub async fn apply(&self, instance_id: &str, snapshot: &mut Snapshot) {
        let mut stats = ProbeStats::default();

        match snapshot {
            Snapshot::Movies(movies) => {
                let groups = movies
                    .iter()
                    .map(|movie| vec![movie.file_path.clone()])
                    .collect();
                let Some(results) = self.probe_groups(instance_id, groups, &mut stats).await else {
                    return;
                };
                for (movie, dates) in movies.iter_mut().zip(results) {
                    movie.added_at = earliest(movie.added_at, oldest(dates));
                }
            }
            Snapshot::Series(series_items) => {
                let groups = series_items
                    .iter()
                    .map(|series| {
                        series
                            .episodes
                            .iter()
                            .map(|episode| episode.file_path.clone())
                            .collect()
                    })
                    .collect();
                let Some(results) = self.probe_groups(instance_id, groups, &mut stats).await else {
                    return;
                };
                for (series, dates) in series_items.iter_mut().zip(results) {
                    for (episode, probed) in series.episodes.iter_mut().zip(dates) {
                        episode.added_at = earliest(episode.added_at, probed);
                    }
                    // Re-roll: the series date is defined as the oldest of its
                    // episodes, and probing can only have lowered those.
                    let oldest_episode = series
                        .episodes
                        .iter()
                        .filter_map(|episode| episode.added_at)
                        .min();
                    series.added_at = earliest(series.added_at, oldest_episode);
                }
            }
            Snapshot::Artists(artists) => {
                let groups = artists
                    .iter()
                    .map(|artist| vec![artist.path.clone()])
                    .collect();
                let Some(results) = self.probe_groups(instance_id, groups, &mut stats).await else {
                    return;
                };
                for (artist, dates) in artists.iter_mut().zip(results) {
                    artist.added_at = earliest(artist.added_at, oldest(dates));
                }
            }
        }

        stats.report(instance_id);
    }

    /// Stats one group of paths per item, keeping the results aligned with the
    /// input so callers can zip them straight back onto their items. `None` is
    /// returned when the whole pass has to be abandoned, which leaves the
    /// snapshot on its *arr dates.
    async fn probe_groups(
        &self,
        instance_id: &str,
        groups: Vec<Vec<Option<String>>>,
        stats: &mut ProbeStats,
    ) -> Option<Vec<Vec<Option<DateTime<Utc>>>>> {
        let mapped = groups
            .into_iter()
            .map(|group| {
                group
                    .into_iter()
                    .map(|raw| self.map_path(raw.as_deref(), stats))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let probe = stream::iter(mapped)
            .map(|group| task::spawn_blocking(move || probe_group(group)))
            .buffered(MAX_CONCURRENT_STATS)
            .collect::<Vec<_>>();

        let Ok(results) = time::timeout(PROBE_TIMEOUT, probe).await else {
            warn!(
                instance_id,
                "media probe: timed out reading media file timestamps; is a network mount unresponsive?"
            );
            return None;
        };

        let mut dates = Vec::with_capacity(results.len());
        for result in results {
            let (group, group_stats) = result.ok()?;
            stats.merge(group_stats);
            dates.push(group);
        }
        Some(dates)
    }

    /// Rewrites an *arr instance's path into this process's view of the
    /// filesystem. Uses component-wise `strip_prefix`, so a rule for `/data`
    /// cannot rewrite `/database/movie.mkv`.
    fn map_path(&self, raw: Option<&str>, stats: &mut ProbeStats) -> Option<PathBuf> {
        let raw = raw.map(str::trim).filter(|raw| !raw.is_empty())?;
        stats.candidates += 1;

        let path = Path::new(raw);
        for mapping in &self.mappings {
            if let Ok(rest) = path.strip_prefix(&mapping.from) {
                return Some(mapping.to.join(rest));
            }
        }

        // No rule matched. Passing the path through unchanged is correct when
        // this process sees media at the same paths the *arr apps do, which is
        // why an empty mapping list is a valid configuration.
        stats.unmapped += 1;
        stats.unmapped_sample.get_or_insert_with(|| raw.to_owned());
        Some(path.to_path_buf())
    }
}

/// Runs on a blocking thread: one task per item, its handful of paths stat'd
/// serially, so a 150-episode series costs one task rather than 150.
fn probe_group(paths: Vec<Option<PathBuf>>) -> (Vec<Option<DateTime<Utc>>>, ProbeStats) {
    let mut stats = ProbeStats::default();
    let dates = paths
        .into_iter()
        .map(|path| {
            let path = path?;
            match probe_path(&path) {
                Ok(date) => {
                    stats.resolved += 1;
                    Some(date)
                }
                Err(miss) => {
                    debug!(path = %path.display(), miss = miss.as_str(), "media probe: no timestamp");
                    stats.record(miss, &path);
                    None
                }
            }
        })
        .collect();
    (dates, stats)
}

/// Follows symlinks, so a symlinked library reports the target's real
/// timestamps and a dangling link is correctly a miss. Both timestamps come
/// from the one `stat`, and creation time wins for a file written years before
/// it was last remuxed or re-tagged.
fn probe_path(path: &Path) -> Result<DateTime<Utc>, ProbeMiss> {
    let metadata = std::fs::metadata(path).map_err(|error| match error.kind() {
        ErrorKind::PermissionDenied => ProbeMiss::Denied,
        _ => ProbeMiss::Missing,
    })?;

    let modified = metadata.modified().ok().and_then(plausible);
    // Unsupported on older kernels, 128-byte-inode ext4, most CIFS and many NFS
    // exports; an error here just means one fewer hint.
    let created = metadata.created().ok().and_then(plausible);

    earliest(modified, created).ok_or(ProbeMiss::Implausible)
}

fn plausible(time: SystemTime) -> Option<DateTime<Utc>> {
    let seconds =
        i64::try_from(time.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs()).ok()?;
    let ceiling = Utc::now().timestamp() + FUTURE_TOLERANCE.as_secs() as i64;
    (EARLIEST_PLAUSIBLE..=ceiling)
        .contains(&seconds)
        .then(|| DateTime::from_timestamp(seconds, 0))
        .flatten()
}

fn oldest(dates: Vec<Option<DateTime<Utc>>>) -> Option<DateTime<Utc>> {
    dates.into_iter().flatten().min()
}

#[derive(Clone, Copy, Debug)]
enum ProbeMiss {
    Missing,
    Denied,
    Implausible,
}

impl ProbeMiss {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Denied => "denied",
            Self::Implausible => "implausible",
        }
    }
}

/// Counters behind the one log line per instance per sync. Without them a
/// mistyped path prefix is indistinguishable from the probe being switched off.
#[derive(Default)]
struct ProbeStats {
    candidates: u64,
    resolved: u64,
    unmapped: u64,
    missing: u64,
    denied: u64,
    implausible: u64,
    unmapped_sample: Option<String>,
    failure_sample: Option<String>,
}

impl ProbeStats {
    fn record(&mut self, miss: ProbeMiss, path: &Path) {
        match miss {
            ProbeMiss::Missing => self.missing += 1,
            ProbeMiss::Denied => self.denied += 1,
            ProbeMiss::Implausible => self.implausible += 1,
        }
        self.failure_sample
            .get_or_insert_with(|| format!("{} ({})", path.display(), miss.as_str()));
    }

    fn merge(&mut self, other: Self) {
        self.candidates += other.candidates;
        self.resolved += other.resolved;
        self.unmapped += other.unmapped;
        self.missing += other.missing;
        self.denied += other.denied;
        self.implausible += other.implausible;
        if self.unmapped_sample.is_none() {
            self.unmapped_sample = other.unmapped_sample;
        }
        if self.failure_sample.is_none() {
            self.failure_sample = other.failure_sample;
        }
    }

    fn report(&self, instance_id: &str) {
        if self.candidates == 0 {
            return;
        }
        if self.unmapped == self.candidates {
            warn!(
                instance_id,
                paths = self.candidates,
                sample = self.unmapped_sample.as_deref().unwrap_or("unknown"),
                "media probe: no path_mappings rule matched any media path; the sample shows the prefix to map from"
            );
            return;
        }
        if self.resolved == 0 {
            warn!(
                instance_id,
                paths = self.candidates,
                sample = self.failure_sample.as_deref().unwrap_or("unknown"),
                "media probe: no media file could be read; check the mount and its permissions"
            );
            return;
        }
        info!(
            instance_id,
            candidates = self.candidates,
            resolved = self.resolved,
            unmapped = self.unmapped,
            missing = self.missing,
            denied = self.denied,
            implausible = self.implausible,
            "media probe finished"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{File, FileTimes},
        time::Duration,
    };

    use chrono::TimeZone;
    use tempfile::tempdir;

    use super::*;
    use crate::collection::{SeriesEpisodeSnapshot, SeriesSnapshot};

    fn probe(mappings: &[(&str, &str)]) -> FileProbe {
        FileProbe {
            mappings: mappings
                .iter()
                .map(|(from, to)| PathMapping {
                    from: PathBuf::from(from),
                    to: PathBuf::from(to),
                })
                .collect(),
        }
    }

    fn map(probe: &FileProbe, raw: &str) -> Option<PathBuf> {
        probe.map_path(Some(raw), &mut ProbeStats::default())
    }

    #[test]
    fn rewrites_with_the_longest_matching_prefix() {
        // Config sorts longest-first; the probe walks the list in order.
        let probe = probe(&[("/data/tv", "/media/tv"), ("/data", "/mnt")]);

        assert_eq!(
            map(&probe, "/data/tv/Show/S01E01.mkv"),
            Some(PathBuf::from("/media/tv/Show/S01E01.mkv"))
        );
        assert_eq!(
            map(&probe, "/data/movies/Film.mkv"),
            Some(PathBuf::from("/mnt/movies/Film.mkv"))
        );
    }

    #[test]
    fn matches_whole_components_only() {
        let probe = probe(&[("/data", "/mnt")]);

        assert_eq!(
            map(&probe, "/database/Film.mkv"),
            Some(PathBuf::from("/database/Film.mkv"))
        );
    }

    #[test]
    fn passes_paths_through_without_mappings() {
        let probe = probe(&[]);

        assert_eq!(
            map(&probe, "/media/tv/Show/S01E01.mkv"),
            Some(PathBuf::from("/media/tv/Show/S01E01.mkv"))
        );
    }

    #[test]
    fn skips_empty_paths_without_counting_a_candidate() {
        let probe = probe(&[]);
        let mut stats = ProbeStats::default();

        assert_eq!(probe.map_path(None, &mut stats), None);
        assert_eq!(probe.map_path(Some("   "), &mut stats), None);
        assert_eq!(stats.candidates, 0);
    }

    fn write_file(path: &Path, modified: SystemTime) {
        let file = File::create(path).expect("create test file");
        file.set_times(FileTimes::new().set_modified(modified))
            .expect("set test file mtime");
    }

    fn seconds_ago(seconds: u64) -> SystemTime {
        SystemTime::now() - Duration::from_secs(seconds)
    }

    #[test]
    fn takes_the_oldest_timestamp_in_a_group() {
        let directory = tempdir().expect("temp dir");
        let old = directory.path().join("old.mkv");
        let new = directory.path().join("new.mkv");
        write_file(&old, seconds_ago(7_200));
        write_file(&new, seconds_ago(60));

        let (dates, stats) = probe_group(vec![Some(new), Some(old)]);

        assert_eq!(stats.resolved, 2);
        assert!(oldest(dates).expect("a date") < Utc::now() - chrono::Duration::hours(1));
    }

    #[test]
    fn an_unreadable_path_costs_a_hint_not_the_group() {
        let directory = tempdir().expect("temp dir");
        let present = directory.path().join("present.mkv");
        write_file(&present, seconds_ago(3_600));

        let (dates, stats) =
            probe_group(vec![Some(directory.path().join("gone.mkv")), Some(present)]);

        assert_eq!(stats.missing, 1);
        assert_eq!(stats.resolved, 1);
        assert!(oldest(dates).is_some());
    }

    #[test]
    fn rejects_implausible_timestamps() {
        assert_eq!(plausible(SystemTime::UNIX_EPOCH), None);
        assert_eq!(
            plausible(SystemTime::now() + Duration::from_secs(30 * 86_400)),
            None
        );
        assert!(plausible(seconds_ago(3_600)).is_some());
    }

    #[test]
    fn never_reports_a_rejected_timestamp() {
        let directory = tempdir().expect("temp dir");
        let epoch = directory.path().join("epoch.mkv");
        let future = directory.path().join("future.mkv");
        write_file(&epoch, SystemTime::UNIX_EPOCH);
        write_file(
            &future,
            SystemTime::now() + Duration::from_secs(30 * 86_400),
        );

        let (dates, _) = probe_group(vec![Some(epoch), Some(future)]);

        // On a filesystem that records creation times these fall back to those;
        // elsewhere they yield nothing. Either way the rejected mtime is gone.
        let floor = Utc.with_ymd_and_hms(1990, 1, 1, 0, 0, 0).unwrap();
        let ceiling = Utc::now() + chrono::Duration::days(1);
        for date in dates.into_iter().flatten() {
            assert!(date > floor && date <= ceiling, "leaked timestamp {date}");
        }
    }

    fn episode(episode_number: i64, file_path: Option<PathBuf>) -> SeriesEpisodeSnapshot {
        SeriesEpisodeSnapshot {
            season_number: 1,
            episode_number,
            title: format!("Episode {episode_number}"),
            air_date_utc: None,
            has_file: file_path.is_some(),
            size_on_disk_bytes: 0,
            added_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
            file_path: file_path.map(|path| path.display().to_string()),
        }
    }

    #[tokio::test]
    async fn lowers_series_and_episode_dates_to_the_oldest_file() {
        let directory = tempdir().expect("temp dir");
        let old = directory.path().join("s01e01.mkv");
        let new = directory.path().join("s01e02.mkv");
        // 2020-06-01T00:00:00Z, comfortably older than the Sonarr date below.
        write_file(
            &old,
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_590_969_600),
        );
        // Newer than the Sonarr date, so it must not raise that episode's age.
        write_file(&new, SystemTime::now());

        let mut snapshot = Snapshot::Series(vec![SeriesSnapshot {
            tvdb_id: 1,
            title: "Show".to_owned(),
            title_slug: "show".to_owned(),
            year: 2020,
            size_on_disk_bytes: 0,
            file_count: 2,
            added_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
            seasons: Vec::new(),
            episodes: vec![episode(1, Some(old)), episode(2, Some(new))],
        }]);

        probe(&[]).apply("sonarr", &mut snapshot).await;

        let Snapshot::Series(series_items) = snapshot else {
            panic!("expected a series snapshot");
        };
        let series = &series_items[0];
        assert_eq!(
            series.episodes[0].added_at,
            Some(Utc.with_ymd_and_hms(2020, 6, 1, 0, 0, 0).unwrap())
        );
        assert_eq!(
            series.episodes[1].added_at,
            Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap())
        );
        assert_eq!(series.added_at, series.episodes[0].added_at);
    }
}
