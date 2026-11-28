//! Human-readable sizes, speeds and times, shared by every list that shows a
//! download. Kept in one place so a row, the details panel and the status bar
//! never disagree about how 1536 looks.

const KB: f64 = 1024.0;
const MB: f64 = KB * 1024.0;
const GB: f64 = MB * 1024.0;

pub fn size(bytes: u64) -> String {
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.2} GB", b / GB)
    }
}

pub fn speed(bps: u64) -> String {
    if bps < 1024 {
        format!("{bps} B/s")
    } else if bps < 1024 * 1024 {
        format!("{:.1} KB/s", bps as f64 / KB)
    } else {
        format!("{:.1} MB/s", bps as f64 / MB)
    }
}

/// Whole-percent progress, or `None` while the size is still unknown.
pub fn percent(received: u64, total: u64) -> Option<u32> {
    if total == 0 {
        return None;
    }
    Some((received.min(total) as f64 / total as f64 * 100.0).round() as u32)
}

/// "42%" or "--" when the total size is not known yet.
pub fn percent_text(received: u64, total: u64) -> String {
    match percent(received, total) {
        Some(value) => format!("{value}%"),
        None => "--".to_string(),
    }
}

/// Time left at the current speed, as "1h 04m", "12m 30s" or "8s". "--" while
/// the size or the speed is unknown, which is the normal state of a magnet
/// link that has not found peers yet.
pub fn eta(received: u64, total: u64, speed_bps: u64) -> String {
    if total == 0 || speed_bps == 0 || received >= total {
        return "--".to_string();
    }
    let seconds = (total - received) / speed_bps;
    if seconds >= 3600 {
        format!("{}h {:02}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// The bit FDM shows under the file name: where it comes from and how far it
/// has got. Speed only while running, ETA while there is a speed to extrapolate
/// from.
pub fn progress_line(received: u64, total: u64, speed_bps: u64, running: bool) -> String {
    let mut parts = Vec::with_capacity(3);
    if total > 0 {
        parts.push(format!("{} / {}", size(received), size(total)));
    } else if received > 0 {
        parts.push(size(received));
    }
    if running && speed_bps > 0 {
        parts.push(speed(speed_bps));
        let eta = eta(received, total, speed_bps);
        if eta != "--" {
            parts.push(format!("{eta} left"));
        }
    }
    parts.join("  ·  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_speeds_and_percentages_read_like_the_list() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1536), "1.5 KB");
        assert_eq!(size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(speed(0), "0 B/s");
        assert_eq!(speed(2 * 1024 * 1024), "2.0 MB/s");
        assert_eq!(percent(50, 200), Some(25));
        assert_eq!(percent(0, 0), None);
        assert_eq!(percent_text(1, 0), "--");
    }

    #[test]
    fn eta_counts_down_only_when_it_can() {
        // 600 bytes to go at 100 B/s.
        assert_eq!(eta(400, 1000, 100), "6s");
        assert_eq!(eta(0, 3600, 1), "1h 00m");
        assert_eq!(eta(0, 120, 1), "2m 00s");
        assert_eq!(eta(0, 0, 100), "--");
        assert_eq!(eta(0, 1000, 0), "--");
        // A complete download has nothing left to wait for.
        assert_eq!(eta(1000, 1000, 100), "--");
    }

    #[test]
    fn the_progress_line_skips_what_it_cannot_know() {
        assert_eq!(progress_line(0, 0, 0, false), "");
        assert_eq!(progress_line(512, 0, 0, true), "512 B");
        assert_eq!(
            progress_line(0, 1024, 1024, true),
            "0 B / 1.0 KB  ·  1.0 KB/s  ·  1s left"
        );
    }
}
