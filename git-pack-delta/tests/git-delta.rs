use rand::RngExt;

fn main() {
    const SAMPLE_DATA: &[u8] = include_bytes!("data/diff-delta.c");
    for _ in 0..20 {
        test_git_delta_index(SAMPLE_DATA);
    }
}

fn create_modified(src: &[u8]) -> Vec<u8> {
    // let mut r = rand::rngs::Xoshiro256PlusPlus::seed_from_u64(42);
    let mut r = rand::rng();
    let mut target = vec![];

    // build randomly modified target from src
    for _ in 0..10 {
        if r.random_bool(0.5) {
            // copy random length from src
            let off = r.random_range(0..src.len());
            let size = r.random_range(1..=src.len() - off);
            target.extend_from_slice(&src[off..off + size]);
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

fn test_git_delta_index(base: &[u8]) {
    assert!(base.len() > 16);
    let target = create_modified(base);

    let index = git_pack_delta::create_delta_index(base).unwrap();
    let delta = git_pack_delta::write_delta_to_vec(base, &index, &target, usize::MAX).unwrap();

    let reconstructed = git_pack_delta::patch_delta(base, &delta).unwrap();

    assert_eq!(target.len(), reconstructed.len());
    assert_eq!(target, reconstructed);
}
