use apple_dust::*;
use rand::{RngExt, SeedableRng};

#[global_allocator]
static ALLOCATOR: AllocTracker<std::alloc::System> = AllocTracker::system();

const SAMPLE_DATA: &[u8] = include_bytes!("../../../git-pack-delta/tests/data/diff-delta.c");

fn create_modified(base: &[u8]) -> Vec<u8> {
    let mut r = rand::rngs::Xoshiro256PlusPlus::seed_from_u64(42);
    let mut target = vec![];

    // build randomly modified target from base
    for _ in 0..10 {
        if r.random_bool(0.5) {
            // copy random length from base
            let off = r.random_range(0..base.len());
            let size = r.random_range(1..=base.len() - off);
            target.extend_from_slice(&base[off..off + size]);
        } else {
            // insert random bytes
            let size = r.random_range(1..=20);
            for _ in 0..size {
                target.push(r.random());
            }
        }
    }
    target
}

fn main() -> Result<(), std::io::Error> {
    let base = SAMPLE_DATA;
    let target = create_modified(base);

    let delta = git_pack_delta::create_delta(base, &target).unwrap();
    let delta = &delta;

    BenchmarkBuilder::default()
        .add_function(
            (base, delta),
            |(base, delta)| git_pack_delta::patch_delta(base, delta).unwrap(),
            "patch_delta",
        )
        .run()
}
