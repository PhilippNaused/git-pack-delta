//! Utilities for LEB128 (Little Endian Base 128) encoding and decoding of integers.

// <https://en.wikipedia.org/wiki/LEB128>

use core::hint::cold_path;
use std::io::Cursor;

const MAX_LEN: usize = 10;
const BIT_MASK: u8 = 1 << 7;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum VarintDecodeError {
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("invalid varint")]
    InvalidData,
}

#[inline]
pub fn encode_varint(value: u64, vec: &mut Vec<u8>) {
    let mut buf = [0u8; MAX_LEN];
    let len = encode_varint_inner(value, &mut buf);
    debug_assert!(len <= MAX_LEN);
    vec.extend_from_slice(&buf[..len]);
}

#[inline]
pub fn get_varint_len(value: u64) -> u32 {
    // TODO: write kani proof for this
    let bits = u64::BITS - value.leading_zeros();
    let len = bits.div_ceil(7).max(1);
    debug_assert!(len > 0);
    debug_assert!(len <= MAX_LEN as u32);
    len
}

#[inline]
const fn encode_varint_inner(mut value: u64, buf: &mut [u8; MAX_LEN]) -> usize {
    let mut i = 0;
    while value >= 0x80 {
        buf[i] = (value as u8) | BIT_MASK;
        value >>= 7;
        i += 1;
    }
    buf[i] = value as u8;
    i + 1
}

pub fn decode_varint(cur: &mut Cursor<&[u8]>) -> core::result::Result<u64, VarintDecodeError> {
    let mut value = 0u64;
    let data = &cur.get_ref()[(cur.position() as usize)..];
    for shift in 0..MAX_LEN {
        if shift >= data.len() {
            return Err(VarintDecodeError::UnexpectedEof);
        }
        let b = data[shift];
        value |= u64::from(b & !BIT_MASK) << (shift * 7);
        if b < BIT_MASK {
            cur.set_position(cur.position() + (shift as u64) + 1);
            return Ok(value);
        }
    }
    cold_path();
    Err(VarintDecodeError::InvalidData)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ITERATIONS: usize = 1000;

    const TEST_DECODE_VALUES32: &[([u8; 5], u64, u64)] = &[
        ([0, 0x80, 0x80, 0x80, 0x80], 0, 1),
        ([127, 0x80, 0x80, 0x80, 0x80], 127, 1),
        ([0x80, 0, 0x80, 0x80, 0x80], 0, 2), // non-canonical encoding of 0
        ([0x80, 1, 0x80, 0x80, 0x80], 128, 2), // canonical encoding of 128
        ([0x80, 0x80, 0x80, 1, 0x80], 1 << 21, 4),
    ];

    #[test]
    fn decode_test_values() {
        for (buf, expect, expect_len) in TEST_DECODE_VALUES32 {
            let mut cur = Cursor::new(buf.as_ref());
            let value = decode_varint(&mut cur).unwrap();
            assert_eq!(value, *expect);
            assert_eq!(cur.position(), *expect_len);
        }
    }

    const BAD_TEST_VALUES64: &[[u8; MAX_LEN]] = &[
        [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80],
        [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0xFF],
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    ];

    const GOOD_TEST_VALUES: &[(u64, &[u8])] = &[
        (0, &[0]),
        (127, &[127]),
        (128, &[0x80, 1]),
        (1 << 21, &[0x80, 0x80, 0x80, 1]),
        (u32::MAX as u64, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]),
        (
            u64::MAX,
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
        ),
    ];

    #[test]
    fn encode_good_values() {
        for &(value, expected) in GOOD_TEST_VALUES {
            let mut buf = vec![];
            encode_varint(value, &mut buf);
            assert_eq!(buf.as_slice(), expected);
            assert_eq!(get_varint_len(value) as usize, buf.len())
        }
    }

    #[test]
    fn bad_values() {
        for &buf in BAD_TEST_VALUES64 {
            let mut cur = Cursor::new(buf.as_ref());
            let res = decode_varint(&mut cur);
            assert!(res.is_err());
            assert_eq!(res.unwrap_err(), VarintDecodeError::InvalidData);
        }
    }

    #[test]
    fn bad_input_eof() {
        for size in 0..64 {
            let value = 1u64 << size;
            let mut buf = vec![];
            encode_varint(value, &mut buf);
            buf.pop(); // simulate EOF by removing the last byte
            let mut cur = Cursor::new(buf.as_ref());
            let res = decode_varint(&mut cur);
            assert!(res.is_err());
            assert_eq!(res.unwrap_err(), VarintDecodeError::UnexpectedEof);
        }
    }

    #[test]
    fn roundtrip() {
        for size in 0..64 {
            let value = 1u64 << size;
            let mut buf = vec![];
            encode_varint(value, &mut buf);
            let len = buf.len();
            let mut cur = Cursor::new(buf.as_ref());
            let decoded = decode_varint(&mut cur).unwrap();
            assert_eq!(len as u64, cur.position());
            assert_eq!(value, decoded);
            assert_eq!(get_varint_len(value) as usize, len)
        }
    }

    #[test]
    fn roundtrip_rand() {
        for value in rand::random_iter().take(TEST_ITERATIONS) {
            let mut buf = vec![];
            encode_varint(value, &mut buf);
            let len = buf.len();
            let mut cur = Cursor::new(buf.as_ref());
            let decoded = decode_varint(&mut cur).unwrap();
            assert_eq!(len as u64, cur.position());
            assert_eq!(value, decoded);
            assert_eq!(get_varint_len(value) as usize, len)
        }
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    pub fn check_encode_varint() {
        let value: u64 = kani::any();
        let mut vec: Vec<u8> = kani::bounded_any::<_, 20>();
        let size = vec.len();
        encode_varint(value, &mut vec);
        assert!(vec.len() <= size + MAX_LEN);
    }

    #[kani::proof]
    pub fn check_roundtrip_varint() {
        let mut vec = vec![];
        let value: u64 = kani::any();
        encode_varint(value, &mut vec);
        assert!(vec.len() <= MAX_LEN);
        assert!(vec.len() > 0);
        let mut cursor = Cursor::new(vec.as_ref());
        let result = decode_varint(&mut cursor);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), value);
        let cur_pos = cursor.position();
        assert_eq!(cur_pos, vec.len() as u64);
    }

    #[kani::proof]
    pub fn check_decode_varint() {
        let data: Vec<u8> = kani::bounded_any::<_, 20>();
        let mut cursor = Cursor::new(data.as_ref());
        let result = decode_varint(&mut cursor);
        if result.is_ok() {
            assert!(cursor.position() > 0);
        } else {
            assert_eq!(cursor.position(), 0);
        }
        assert!(cursor.position() <= MAX_LEN as u64);
    }
}
