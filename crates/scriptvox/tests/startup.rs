mod common;
use std::time::{Duration, Instant};

/// Startup budget (PLAN-R1 step 0). Target about 50 ms on WSL; the hard limit
/// here is looser so a busy shared machine does not make the test flaky.
#[test]
fn status_starts_fast() {
    let best = (0..5)
        .map(|_| {
            let t = Instant::now();
            let ok = common::std_bin()
                .args(["status", "--json"])
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok);
            t.elapsed()
        })
        .min()
        .unwrap();
    eprintln!("scriptvox status --json best of 5: {best:?}");
    assert!(
        best < Duration::from_millis(200),
        "startup {best:?} is over budget"
    );
}
