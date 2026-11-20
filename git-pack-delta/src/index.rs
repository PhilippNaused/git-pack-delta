//! <https://github.com/git/git/blob/master/diff-delta.c>

use crate::hash::RollingHash;
use core::cmp;

/// maximum hash entry list for the same hash bucket
const HASH_LIMIT: usize = 64;

/// Entry in the rabin hash table
#[derive(Debug)]
pub(crate) struct IndexEntry<H: RollingHash> {
    /// offset into the buffer
    pub(crate) offset: u32,
    /// rabin fingerprint of the values starting at `offset`
    pub(crate) hash: H::H,
}

impl<H: RollingHash> Copy for IndexEntry<H> {}

impl<H: RollingHash> Clone for IndexEntry<H> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Debug)]
pub struct DeltaIndex<H: RollingHash> {
    pub(crate) entries: Vec<Vec<IndexEntry<H>>>,
    pub(crate) hash_mask: u32,
}

pub fn create_delta_index_custom<H: RollingHash>(base: &[u8]) -> Option<DeltaIndex<H>> {
    let base_len = base.len();
    if base_len == 0 {
        return None;
    }
    // Current delta format can't encode offsets into reference buffer with more than 32 bits.
    const MAX_SIZE: usize = u32::MAX as usize;
    let entries_count = ((cmp::min(base_len, MAX_SIZE) - 1) / H::WINDOW) as u32;
    if entries_count == 0 {
        return None;
    }
    let h_size = get_h_size(entries_count);
    debug_assert!(h_size.is_power_of_two());
    let hash_mask = h_size - 1;

    let mut entries: Vec<Vec<IndexEntry<H>>> = vec![Vec::new(); h_size as usize];
    // let hash_count = vec![0; h_size];

    let mut prev_hash = H::empty();
    let stride = H::WINDOW as u32;

    // iterate backward over the source buffer and fill the hash table with fingerprints
    // TODO: why backwards???
    let mut pos: u32 = (entries_count - 1) * H::WINDOW as u32;
    loop {
        let window_start = (pos + 1) as usize; // why +1???
        let window_end = window_start + H::WINDOW;
        debug_assert!(window_end <= base_len);
        let window = &base[window_start..window_end];
        let offset = pos + H::WINDOW as u32;
        let hash = H::value_from_slice(window);
        let index = H::apply_mask(hash, hash_mask); // index in the hash table
        if hash == prev_hash {
            // keep the lowest of consecutive identical blocks
            if let Some(last) = entries[index].last_mut() {
                last.offset = offset;
            }
        } else {
            prev_hash = hash;
            entries[index].push(IndexEntry { offset, hash });
        }
        if pos == 0 {
            break;
        }
        pos = pos.saturating_sub(stride);
    }

    // cull the hash table
    for bucket in entries.iter_mut() {
        cull_bucket(bucket);
    }

    Some(DeltaIndex { entries, hash_mask })
}

/// Culls a hash table bucket to ensure it contains at most [`HASH_LIMIT`] entries.
/// Keeps evenly distributed entries from the original bucket.
fn cull_bucket<T: Copy>(bucket: &mut Vec<T>) {
    if bucket.len() <= HASH_LIMIT {
        return;
    }
    let mut keep = Vec::with_capacity(HASH_LIMIT);
    for i in 0..HASH_LIMIT {
        let idx = i * bucket.len() / HASH_LIMIT;
        keep.push(bucket[idx]);
    }
    *bucket = keep;
}

/// Returns the smallest power of two that is >= max(entries_count / 4, 16)
fn get_h_size(entries_count: u32) -> u32 {
    // git uses a for loop for this.
    // this only has one branch.
    let m = cmp::max(entries_count / 4, 16);
    if m.is_power_of_two() {
        core::hint::cold_path();
        m
    } else {
        m.isolate_highest_one() << 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE_DATA: &[u8] = include_bytes!("../tests/data/diff-delta.c");

    #[test]
    fn new_delta_index_git_rabin() {
        new_delta_index_inner::<crate::hash::GitRabin>();
    }

    #[test]
    fn new_delta_index_cyclic_poly32() {
        new_delta_index_inner::<crate::hash::CyclicPoly32>();
    }

    #[test]
    fn new_delta_index_cyclic_poly64() {
        new_delta_index_inner::<crate::hash::CyclicPoly64>();
    }

    fn new_delta_index_inner<H: RollingHash>() {
        let base = SAMPLE_DATA;
        let index = create_delta_index_custom::<H>(base).unwrap();
        assert!((index.hash_mask + 1).is_power_of_two());

        // check that entries are sorted by offset (descending)
        for bucket in &index.entries {
            for i in 1..bucket.len() {
                assert!(bucket[i - 1].offset >= bucket[i].offset);
            }
        }
    }

    #[test]
    fn cull_bucket_small() {
        let mut bucket: Vec<_> = (0..HASH_LIMIT).collect();
        cull_bucket(&mut bucket);
        assert!(bucket.len() == HASH_LIMIT);
    }

    #[test]
    fn cull_bucket_large() {
        let factor = 7;
        let mut bucket: Vec<_> = (0..(HASH_LIMIT * factor)).collect();
        cull_bucket(&mut bucket);
        let expected: Vec<_> = (0..HASH_LIMIT).map(|i| i * factor).collect();
        assert!(bucket.len() == HASH_LIMIT);
        assert_eq!(bucket, expected);
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    const MAX_ENTRIES: u32 = (u32::MAX - 1) / 16;

    #[kani::proof]
    #[kani::unwind(1)]
    fn check_get_h_size() {
        let entries_count = kani::any();
        // kani::assume(entries_count > 0);
        // kani::assume(entries_count <= MAX_ENTRIES);
        let h_size = get_h_size(entries_count);
        assert!(h_size.is_power_of_two());
        let min = cmp::max(entries_count / 4, 16);
        assert!(h_size >= min);
        assert!((h_size / 2) < min);
    }
}
