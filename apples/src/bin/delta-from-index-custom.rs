use apple_dust::*;
use git_pack_delta::hash;
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

    let index1 = git_pack_delta::create_delta_index_custom::<hash::GitRabin>(base).unwrap();
    let index2 = git_pack_delta::create_delta_index_custom::<hash::CyclicPoly32>(base).unwrap();
    let index3 = git_pack_delta::create_delta_index_custom::<hash::CyclicPoly64>(base).unwrap();

    BenchmarkBuilder::default()
        .add_function(
            (base, &target, &index1),
            |(src, target, index)| {
                git_pack_delta::write_delta_to_vec(src, index, target, usize::MAX).unwrap()
            },
            "GitRabin",
        )
        .add_function(
            (base, &target, &index2),
            |(src, target, index)| {
                git_pack_delta::write_delta_to_vec(src, index, target, usize::MAX).unwrap()
            },
            "CyclicPoly32",
        )
        .add_function(
            (base, &target, &index3),
            |(src, target, index)| {
                git_pack_delta::write_delta_to_vec(src, index, target, usize::MAX).unwrap()
            },
            "CyclicPoly64",
        )
        .add_function(
            (base, &target, &index1),
            |(src, target, index)| {
                git_pack_delta::get_delta_len(src, index, target, usize::MAX).unwrap()
            },
            "GitRabin_len",
        )
        .run()
}
