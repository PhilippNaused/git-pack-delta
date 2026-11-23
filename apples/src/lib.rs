use rand::{RngExt, SeedableRng};

pub const SAMPLE_DATA: &[u8] = include_bytes!("../../git-pack-delta/tests/data/diff-delta.c");
pub const DEFAULT_DIFFERENCE: f64 = 0.05;

/// Produces a reproducible revision with insertions, deletions and replacements.
/// `difference` is the probability of editing each chunk, between 0 and 1 inclusive;
/// use [`DEFAULT_DIFFERENCE`] for the default rate.
/// Inserted and replacement bytes are sampled from the base data.
///
/// # Panics
/// Panics if `difference` is not finite or is outside the range 0 to 1.
pub fn create_modified(base: &[u8], difference: f64) -> Vec<u8> {
    assert!(
        (0.0..=1.0).contains(&difference),
        "difference must be between 0 and 1"
    );
    let mut r = rand::rngs::Xoshiro256PlusPlus::seed_from_u64(42);
    let mut target = Vec::with_capacity(base.len());
    for chunk in base.chunks(16) {
        if !r.random_bool(difference) {
            target.extend_from_slice(chunk);
            continue;
        }

        match r.random_range(0..3) {
            0 => {}
            edit => {
                let source = r.random_range(0..=base.len() - chunk.len());
                let replacement = &base[source..source + chunk.len()];
                target.extend_from_slice(replacement);
                if edit == 1 {
                    target.extend_from_slice(chunk);
                } else if replacement == chunk {
                    *target.last_mut().unwrap() ^= 1;
                }
            }
        }
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modified_sample_is_reproducible_and_delta_compressible() {
        let target = create_modified(SAMPLE_DATA, DEFAULT_DIFFERENCE);
        assert_eq!(target, create_modified(SAMPLE_DATA, DEFAULT_DIFFERENCE));
        assert_ne!(target, SAMPLE_DATA);
        assert!(target.len().abs_diff(SAMPLE_DATA.len()) < SAMPLE_DATA.len() / 20);

        let delta = git_pack_delta::create_delta(SAMPLE_DATA, &target).unwrap();
        assert!(delta.len() < target.len() / 10);
        assert_eq!(
            git_pack_delta::patch_delta(SAMPLE_DATA, &delta).unwrap(),
            target
        );
    }

    #[test]
    fn modified_handles_empty_and_short_inputs() {
        for size in 0..=256 {
            let base = vec![b'a'; size];
            for difference in [0.0, DEFAULT_DIFFERENCE, 1.0] {
                assert_eq!(
                    create_modified(&base, difference),
                    create_modified(&base, difference)
                );
            }
        }
    }

    #[test]
    fn difference_controls_modifications() {
        assert_eq!(create_modified(SAMPLE_DATA, 0.0), SAMPLE_DATA);
        let default = create_modified(SAMPLE_DATA, DEFAULT_DIFFERENCE);
        let half_modified = create_modified(SAMPLE_DATA, 0.5);
        let fully_modified = create_modified(SAMPLE_DATA, 1.0);
        assert_ne!(fully_modified, SAMPLE_DATA);
        let default_delta = git_pack_delta::create_delta(SAMPLE_DATA, &default).unwrap();
        let half_delta = git_pack_delta::create_delta(SAMPLE_DATA, &half_modified).unwrap();
        let full_delta = git_pack_delta::create_delta(SAMPLE_DATA, &fully_modified).unwrap();
        assert!(half_delta.len() > default_delta.len());
        assert!(full_delta.len() > half_delta.len());
        assert_eq!(
            git_pack_delta::patch_delta(SAMPLE_DATA, &full_delta).unwrap(),
            fully_modified
        );
    }
}
