//! Time-range chunks (PLAN-R1 §5; ideas from RM #11864, HF #1231, HF #6029):
//! a long render is cut into video-only chunks of `per` frames, so a killed
//! or stalled render resumes from the last finished chunk.

/// Frames per chunk: `SCRIPTVOX_CHUNK_S` seconds (default 10) at `fps`.
pub fn per_chunk(fps: u32) -> u64 {
    let secs: f64 = std::env::var("SCRIPTVOX_CHUNK_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|s: &f64| *s > 0.0)
        .unwrap_or(10.0);
    ((secs * f64::from(fps)).round() as u64).max(1)
}

/// Half-open frame ranges `[a, b)` that cover `0..total` in order.
pub fn ranges(total: u64, per: u64) -> Vec<(u64, u64)> {
    (0..total.div_ceil(per))
        .map(|i| (i * per, ((i + 1) * per).min(total)))
        .collect()
}

/// The `-segment_frames` list for a render that starts at `start`: chunk
/// boundaries relative to the start.
pub fn segment_frames(start: u64, total: u64, per: u64) -> Vec<u64> {
    ranges(total, per)
        .into_iter()
        .map(|(_, b)| b)
        .filter(|b| *b > start && *b < total)
        .map(|b| b - start)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples() {
        assert_eq!(ranges(10, 4), vec![(0, 4), (4, 8), (8, 10)]);
        assert_eq!(segment_frames(0, 10, 4), vec![4, 8]);
        assert_eq!(segment_frames(4, 10, 4), vec![4]);
        assert_eq!(ranges(0, 4), vec![]);
    }
}
