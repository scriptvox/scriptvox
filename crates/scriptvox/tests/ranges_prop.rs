use proptest::prelude::*;
use scriptvox::ranges::{ranges, segment_frames};

proptest! {
    #[test]
    fn ranges_cover_the_timeline_without_overlap(total in 0u64..100_000, per in 1u64..2_000) {
        let r = ranges(total, per);
        let mut next = 0;
        for (a, b) in &r {
            prop_assert_eq!(*a, next);
            prop_assert!(b > a && b - a <= per);
            next = *b;
        }
        prop_assert_eq!(next, total);
    }

    #[test]
    fn a_resume_cuts_at_the_same_boundaries(total in 1u64..20_000, per in 1u64..500, k in 0u64..50) {
        let start = (k * per).min(total);
        let full: Vec<u64> = segment_frames(0, total, per);
        let resumed: Vec<u64> = segment_frames(start, total, per).into_iter().map(|b| b + start).collect();
        prop_assert!(resumed.iter().all(|b| full.contains(b)));
    }
}
