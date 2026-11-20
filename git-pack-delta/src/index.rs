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
    pub(crate) entries: Vec<IndexEntry<H>>,
    pub(crate) fan_out: Vec<u32>,
    pub(crate) hash_mask: u32,
}

#[derive(Debug)]
struct UnpackedEntry<H: RollingHash> {
    entry: IndexEntry<H>,
    next: Option<u32>,
}

#[derive(Debug)]
struct IndexBuilder<H: RollingHash> {
    h_size: usize,
    /// linked lists of entries
    list: Vec<UnpackedEntry<H>>,
    /// start index of each linked list
    starts: Vec<Option<u32>>,
    /// length of each linked list
    counts: Vec<u32>,
}

impl<H: RollingHash> IndexBuilder<H> {
    fn new(size: usize, h_size: usize) -> Self {
        Self {
            h_size,
            list: Vec::with_capacity(size),
            starts: vec![None; h_size],
            counts: vec![0; h_size + 1],
        }
    }

    /// Limit the number of entries in the same hash bucket to [`HASH_LIMIT`].
    /// This guards us against pathological data sets causing really bad hash distribution with most entries in the same hash
    /// bucket that would bring us to `O(m*n)` computing costs (`m` and `n` corresponding to reference and target buffer sizes).
    /// Make sure none of the hash buckets has more entries than we're willing to test.
    /// Otherwise we cull the entry list uniformly to still preserve a good repartition across the reference buffer.
    fn cull(&mut self) {
        for i in 0..self.h_size {
            if self.counts[i] <= HASH_LIMIT as u32 {
                continue;
            }

            let mut acc: i32 = 0;
            let mut entry = self.starts[i];

            // Assume that this loop is gone through exactly `HASH_LIMIT` times and is entered and left with `acc==0`.
            // So the first statement in the loop contributes `(counts[i]-HASH_LIMIT)*HASH_LIMIT` to the accumulator,
            // and the inner loop consequently is run `counts[i]-HASH_LIMIT` times,
            // removing one element from the list each time.
            // Since acc balances out to `0` at the final run,
            // the inner loop body can't be left with `entry==None`.
            // So we indeed encounter `entry==None` in the outer loop only.
            while entry.is_some() {
                acc += self.counts[i] as i32 - HASH_LIMIT as i32;
                if acc > 0 {
                    let keep = entry.unwrap();
                    while acc > 0 {
                        entry = self.list[entry.unwrap() as usize].next;
                        acc -= HASH_LIMIT as i32;
                    }
                    self.list[keep as usize].next = entry;
                }
                entry = self.list[entry.unwrap() as usize].next;
            }
            debug_assert_eq!(acc, 0);
        }
    }

    fn build(mut self) -> DeltaIndex<H> {
        self.cull();

        debug_assert_eq!(self.h_size + 1, self.counts.len());
        debug_assert_eq!(self.h_size, self.starts.len());

        let h_size = self.h_size;
        let mut fan_out = self.counts; // re-use the now unused vec as the fan-out table.
        fan_out.fill(0);

        let mut packed_hashes = Vec::with_capacity(self.list.len());
        for i in 0..h_size {
            // Coalesce all entries belonging to one linked list into consecutive array entries.
            fan_out[i] = packed_hashes.len() as u32;
            let mut next_idx = self.starts[i];
            while let Some(idx) = next_idx {
                let e = &self.list[idx as usize];
                packed_hashes.push(e.entry);
                next_idx = e.next;
            }
        }
        debug_assert_eq!(packed_hashes.len(), self.list.len());
        fan_out[h_size] = packed_hashes.len() as u32; // Sentinel value to indicate the length of the last hash bucket

        DeltaIndex {
            entries: packed_hashes,
            fan_out,
            hash_mask: (h_size - 1) as u32,
        }
    }
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
    let h_size = get_h_size(entries_count) as usize;
    debug_assert!(h_size.is_power_of_two());
    let hash_mask = (h_size - 1) as u32;

    // linked lists of entries
    let mut builder = IndexBuilder::new(entries_count as usize, h_size);

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
        let h_index = H::apply_mask(hash, hash_mask); // index in the hash table
        if hash == prev_hash {
            // keep the lowest of consecutive identical blocks
            if let Some(last) = builder.list.last_mut() {
                debug_assert_eq!(last.entry.hash, hash);
                last.entry.offset = offset;
            }
        } else {
            prev_hash = hash;
            let last_idx = builder.starts[h_index];
            if let Some(idx) = last_idx {
                debug_assert_eq!(
                    H::apply_mask(builder.list[idx as usize].entry.hash, hash_mask),
                    h_index
                );
            }
            builder.starts[h_index] = Some(builder.list.len() as u32);
            builder.list.push(UnpackedEntry {
                entry: IndexEntry { offset, hash },
                next: last_idx,
            });
            builder.counts[h_index] += 1;
        }
        if pos == 0 {
            break;
        }
        pos = pos.saturating_sub(stride);
    }

    Some(builder.build())
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
        // for bucket in &index.entries {
        //     for i in 1..bucket.len() {
        //         assert!(bucket[i - 1].offset >= bucket[i].offset);
        //     }
        // }
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
