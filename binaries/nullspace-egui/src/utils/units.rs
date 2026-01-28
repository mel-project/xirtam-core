pub fn unit_for_bytes(bytes: u64) -> (f64, &'static str) {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    const TB: f64 = 1_000_000_000_000.0;

    let bytes = bytes as f64;
    if bytes >= TB {
        (TB, "TB")
    } else if bytes >= GB {
        (GB, "GB")
    } else if bytes >= MB {
        (MB, "MB")
    } else if bytes >= KB {
        (KB, "KB")
    } else {
        (1.0, "B")
    }
}

pub fn format_filesize(bytes: u64, unit_scale: f64) -> String {
    if unit_scale <= 1.0 {
        format!("{bytes}")
    } else {
        format!("{:.1}", bytes as f64 / unit_scale)
    }
}
