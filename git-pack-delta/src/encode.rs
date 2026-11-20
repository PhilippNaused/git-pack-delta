//! <https://github.com/git/git/blob/master/diff-delta.c>

use crate::{COPY_CMD_FLAG, ZERO_COPY_LEN};
use crate::{hash::RollingHash, index::DeltaIndex, match_bytes, varint};
use core::cmp;

/// Maximum length of an insert instruction
const MAX_INSERT_LEN: usize = 127;
/// Maximum length of a copy instruction
const MAX_COPY_LEN: u32 = (1 << (3 * 8)) - 1; // 64 KiB -1

/// A writer for delta instructions
pub trait DeltaWriter {
    /// Emits a length prefix for either the base or target.
    fn emit_len(&mut self, value: usize, base: bool);
    /// Emits a literal insert instruction with the given data.
    fn emit_literal(&mut self, data: &[u8]);
    /// Emits a copy instruction with the given offset and length.
    fn emit_copy(&mut self, offset: u32, length: u32);
    /// Returns the total length of the emitted delta instructions.
    fn len(&self) -> usize;
}

/// Returns the length of the delta encoding for the given base and target using the specified index.
/// If the length of the delta would exceed `max_output_len`, returns [`None`] instead.
/// This is ~10-20% faster than actually writing the delta and requires no memory allocations.
pub fn get_delta_len<H: RollingHash>(
    base: &[u8],
    index: &DeltaIndex<H>,
    target: &[u8],
    max_output_len: usize,
) -> Option<usize> {
    let mut writer = LengthWriter::default();
    write_delta_to(base, index, target, max_output_len, &mut writer)?;
    Some(writer.0)
}

pub fn write_delta_to<H: RollingHash>(
    base: &[u8],
    index: &DeltaIndex<H>,
    target: &[u8],
    max_output_len: usize,
    writer: &mut impl DeltaWriter,
) -> Option<()> {
    let base_len = base.len();
    let target_len = target.len();
    if target_len == 0 {
        return None;
    }
    debug_assert_eq!(writer.len(), 0);

    writer.emit_len(base_len, true);
    writer.emit_len(target_len, false);

    let initial_size = cmp::min(H::WINDOW, target_len);
    let initial_data = &target[..initial_size];
    // initialize the rolling hash:
    let mut hasher = H::from_slice(initial_data);
    let mut insert_count = initial_size; // size of pending insert command
    let mut target_pos = initial_size; // ptr in target
    let mut match_off: u32 = 0; // offset of current match
    let mut match_len: u32 = 0; // length of current match

    const GOOD_MATCH_LENGTH: u32 = 4096; // good enough

    while target_pos < target_len {
        debug_assert!(target_pos >= H::WINDOW);
        if match_len < GOOD_MATCH_LENGTH {
            // update rolling hash
            H::roll_once(
                &mut hasher,
                target[target_pos],
                target[target_pos - H::WINDOW],
            );
            // find match
            for ref_start in index.iter_offsets(hasher.value()) {
                let new_len =
                    match_bytes::count_matches(&base[ref_start as usize..], &target[target_pos..])
                        as u32;
                if new_len > match_len {
                    // found a better match
                    match_off = ref_start;
                    match_len = new_len;
                    if match_len >= GOOD_MATCH_LENGTH {
                        break; // good enough
                    }
                }
            }
        }

        if match_len < 4 {
            // bad match => use insert command instead
            insert_count += 1;
            target_pos += 1;
            debug_assert!(insert_count <= MAX_INSERT_LEN);
            if insert_count == MAX_INSERT_LEN {
                // flush insert command when it reaches max length
                writer.emit_literal(&target[target_pos - insert_count..target_pos]);
                insert_count = 0;
            }
            match_len = 0; // reset match size for next iteration
        } else {
            // good match
            // try to back-extend the match to include the pending insert bytes
            let extend = match_bytes::count_matches_rev(
                &base[..match_off as usize],
                &target[..target_pos],
                insert_count,
            );
            // dbg!(extend);

            if extend > 0 {
                match_off -= extend as u32;
                match_len += extend as u32;
                insert_count -= extend;
                target_pos -= extend;

                if insert_count > 0 {
                    // flush remaining insert command
                    writer.emit_literal(&target[target_pos - insert_count..target_pos]);
                    insert_count = 0;
                }
            }

            // git still uses the v2 format where copy instructions are limited to 64KiB (ZERO_COPY_LEN).
            // but we use v3, so a copy can be up to 16MiB (MAX_COPY_LEN)
            let copy_length = cmp::min(match_len, MAX_COPY_LEN);

            writer.emit_copy(match_off, copy_length);
            target_pos += copy_length as usize;
            match_off = match_off.saturating_add(copy_length);
            match_len -= copy_length;
            if match_off == u32::MAX {
                match_len = 0;
            }
            if match_len < GOOD_MATCH_LENGTH {
                // re-seed hash
                debug_assert!(target_pos >= H::WINDOW);
                let window = &target[target_pos - H::WINDOW..target_pos];
                hasher.reset_with(window);
            }
        }

        if writer.len() > max_output_len {
            return None;
        }
    }

    debug_assert!(insert_count <= MAX_INSERT_LEN);
    if insert_count > 0 {
        // flush remaining insert instruction
        if writer.len() + insert_count + 1 > max_output_len {
            return None;
        }
        writer.emit_literal(&target[target_len - insert_count..target_len]);
    } else {
        if writer.len() > max_output_len {
            return None;
        }
    }

    debug_assert!(writer.len() <= max_output_len);
    Some(())
}

impl DeltaWriter for Vec<u8> {
    #[inline]
    fn emit_literal(&mut self, data: &[u8]) {
        let len = data.len();
        assert!(len > 0);
        assert!(len <= MAX_INSERT_LEN);
        self.push(len as u8);
        self.extend_from_slice(data);
    }
    fn emit_copy(&mut self, offset: u32, length: u32) {
        debug_assert!(length > 0);
        debug_assert!(length <= MAX_COPY_LEN);
        let mut value = [0u8; 8];
        value[0] = COPY_CMD_FLAG; // copy flag
        let mut i = 1;

        // the compiler will unroll these loops and skip the bounds checks :)
        for o in 0..4 {
            if offset & (0xff << (o * 8)) != 0 {
                value[i] = (offset >> (o * 8)) as u8;
                i += 1;
                value[0] |= 1 << o;
            }
        }

        // using 3 bits for the length is only supported in v3 of git's delta format.
        // but git has only been using v2 since 2006. (it can read the newer format, though)
        if length != ZERO_COPY_LEN {
            for l in 0..3 {
                if length & (0xff << (l * 8)) != 0 {
                    value[i] = (length >> (l * 8)) as u8;
                    i += 1;
                    value[0] |= 0x10 << l;
                }
            }
        }

        debug_assert!(i <= 1 + 4 + 3);
        debug_assert!(i > 0);

        self.extend_from_slice(&value[..i]);
    }
    #[inline]
    fn len(&self) -> usize {
        self.len()
    }
    #[inline]
    fn emit_len(&mut self, value: usize, _base: bool) {
        varint::encode_varint(value as u64, self);
    }
}

#[derive(Debug, Default)]
struct LengthWriter(usize);

impl DeltaWriter for LengthWriter {
    fn emit_len(&mut self, value: usize, _base: bool) {
        self.0 += varint::get_varint_len(value as u64) as usize;
    }
    #[inline]
    fn emit_literal(&mut self, data: &[u8]) {
        self.0 += data.len() + 1;
    }
    fn emit_copy(&mut self, offset: u32, length: u32) {
        debug_assert!(length > 0);
        debug_assert!(length <= MAX_COPY_LEN);
        let mut i = 1;

        // the compiler will unroll these loops and skip the bounds checks :)
        for o in 0..4 {
            if offset & (0xff << (o * 8)) != 0 {
                i += 1;
            }
        }

        // using 3 bits for the length is only supported in v3 of git's delta format.
        // but git has only been using v2 since 2006. (it can read the newer format, though)
        if length != ZERO_COPY_LEN {
            for l in 0..3 {
                if length & (0xff << (l * 8)) != 0 {
                    i += 1;
                }
            }
        }
        debug_assert!(i <= 1 + 4 + 3);
        debug_assert!(i > 0);
        self.0 += i;
    }
    #[inline]
    fn len(&self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_copy_instruction() {
        let data: Vec<(u32, u32, &[u8])> = vec![
            (0, ZERO_COPY_LEN, &[0b1000_0000]),
            (
                0x78563412,
                ZERO_COPY_LEN,
                &[0b1000_1111, 0x12, 0x34, 0x56, 0x78],
            ),
            (0x78003400, ZERO_COPY_LEN, &[0b1000_1010, 0x34, 0x78]),
            (0, 0xAB, &[0b1001_0000, 0xAB]),
            (0, 0xabcdef, &[0b1111_0000, 0xef, 0xcd, 0xab]),
            (
                0x78563412,
                0xabcdef,
                &[0b1111_1111, 0x12, 0x34, 0x56, 0x78, 0xef, 0xcd, 0xab],
            ),
            (0, MAX_COPY_LEN, &[0b1111_0000, 0xff, 0xff, 0xff]),
        ];
        for (offset, length, expected) in data {
            let mut out = Vec::new();
            out.emit_copy(offset, length);
            assert_eq!(out, expected);
        }
    }
}
