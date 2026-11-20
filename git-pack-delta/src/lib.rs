//! Creates a delta encoding that can be used to create `target` from `base`.
//! ```
//! let base = "This is a pure rust implementation of Git's delta compression algorithm.";
//! let target = base.replace("Git", "git");
//! let delta = git_pack_delta::create_delta(base.as_bytes(), target.as_bytes()).unwrap();
//! assert_eq!(delta.len(), 9);
//! let target2 = git_pack_delta::patch_delta(base.as_bytes(), &delta).unwrap();
//! assert_eq!(target.as_bytes(), target2);
//! ```
//! See: <https://github.com/git/git/blob/master/diff-delta.c>
//! and <https://github.com/git/git/blob/master/patch-delta.c>

#[macro_use]
extern crate static_assertions_next;

mod decode;
mod encode;
pub mod hash;
mod index;
pub mod match_bytes;
pub mod varint;

const COPY_CMD_FLAG: u8 = 0x80;
const ZERO_COPY_LEN: u32 = 0x10000;

pub use decode::DiffDecodeError;
pub use encode::get_delta_len;
pub use encode::write_delta_to;
pub use hash::RollingHash;
pub use index::DeltaIndex;
pub use index::create_delta_index_custom;

pub fn write_delta_to_vec<H: RollingHash>(
    base: &[u8],
    index: &DeltaIndex<H>,
    target: &[u8],
    max_output_len: usize,
) -> Option<Vec<u8>> {
    const MAX_INITIAL_CAP: usize = 8192; // same as git
    let mut initial_cap = MAX_INITIAL_CAP.min(target.len() / 2);
    let max_op_size = 5 + 5 + 1 + H::WINDOW + 7;
    if initial_cap >= max_output_len {
        initial_cap = max_output_len + max_op_size + 1;
    }

    let mut output = Vec::with_capacity(max_output_len.min(initial_cap));
    write_delta_to(base, index, target, max_output_len, &mut output)?;
    debug_assert!(output.len() <= max_output_len);
    #[cfg(debug_assertions)]
    {
        let len = get_delta_len(base, index, target, max_output_len).unwrap();
        assert_eq!(output.len(), len);
    }
    Some(output)
}

pub fn create_delta_index(base: &[u8]) -> Option<DeltaIndex<hash::GitRabin>> {
    create_delta_index_custom::<hash::GitRabin>(base)
}

/// Creates a delta encoding that can be used to create `target` from `base`.
/// ```
/// let base = "This is a pure rust implementation of Git's delta compression algorithm.";
/// let target = base.replace("Git", "git");
/// let delta = git_pack_delta::create_delta(base.as_bytes(), target.as_bytes()).unwrap();
/// assert_eq!(delta.len(), 9);
/// ```
pub fn create_delta(base: &[u8], target: &[u8]) -> Option<Vec<u8>> {
    if target.len() < 18 {
        return None;
    }
    let index = create_delta_index(base)?;
    let _max_size = target.len() - 1;
    write_delta_to_vec(base, &index, target, usize::MAX)
}

/// Recreates the target from the `base` and encoded `delta`.
/// ```
/// let base = "This is a pure rust implementation of Git's delta compression algorithm.";
/// let target = base.replace("Git", "git");
/// let delta = git_pack_delta::create_delta(base.as_bytes(), target.as_bytes()).unwrap();
/// let target2 = git_pack_delta::patch_delta(base.as_bytes(), &delta).unwrap();
/// assert_eq!(target.as_bytes(), target2);
/// ```
pub fn patch_delta(base: &[u8], delta: &[u8]) -> Result<Vec<u8>, DiffDecodeError> {
    let mut data = vec![];
    decode::decode(base, delta, &mut data)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DATA: &str = include_str!("../tests/data/diff-delta.c");

    #[test]
    fn test_round_trip_rabin() {
        test_round_trip::<hash::GitRabin>(130);
    }

    #[test]
    fn test_round_trip_cyclic_poly32() {
        test_round_trip::<hash::CyclicPoly32>(130);
    }

    #[test]
    fn test_round_trip_cyclic_poly64() {
        test_round_trip::<hash::CyclicPoly64>(130);
    }

    #[test]
    fn example1() {
        let base = "This is a pure rust implementation of Git's delta compression algorithm.";
        let target = base.replace("Git", "git");
        let delta = create_delta(base.as_bytes(), target.as_bytes()).unwrap();
        assert_eq!(delta.len(), 9);
        let target2 = patch_delta(base.as_bytes(), &delta).unwrap();
        assert_eq!(target.as_bytes(), target2);
    }

    fn test_round_trip<R: RollingHash>(exp: usize) {
        let base = SAMPLE_DATA.as_bytes();
        let target = SAMPLE_DATA.to_owned().replace("RABIN", "ROBIN");
        let target = target.as_bytes();

        let index = create_delta_index_custom::<R>(base).unwrap();
        let delta = write_delta_to_vec(base, &index, target, usize::MAX).unwrap();
        let decoded = patch_delta(base, &delta).unwrap();
        assert_eq!(decoded, target);
        assert_eq!(delta.len(), exp);
    }
}
