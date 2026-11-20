//! <https://github.com/git/git/blob/master/patch-delta.c>

use super::{COPY_CMD_FLAG, ZERO_COPY_LEN};
use crate::varint::{VarintDecodeError, decode_varint};
use core::hint::cold_path;
use std::collections::TryReserveError;
use std::io::Cursor;

type Result<T> = core::result::Result<T, DiffDecodeError>;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum DiffDecodeError {
    #[error("buffer doesn't fit in memory")]
    OutOfMemory,
    #[error("base size mismatch: {0} != {1}")]
    BaseSizeMismatch(usize, usize),
    #[error("result size mismatch: {0} != {1}")]
    ResultSizeMismatch(usize, usize),
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("invalid data")]
    InvalidData,
}

impl From<DiffDecodeError> for std::io::Error {
    fn from(value: DiffDecodeError) -> Self {
        let kind = match value {
            DiffDecodeError::OutOfMemory => std::io::ErrorKind::OutOfMemory,
            DiffDecodeError::UnexpectedEof => std::io::ErrorKind::UnexpectedEof,
            _ => std::io::ErrorKind::InvalidData,
        };
        Self::new(kind, value)
    }
}
impl From<TryReserveError> for DiffDecodeError {
    fn from(_: TryReserveError) -> Self {
        Self::OutOfMemory
    }
}
impl From<VarintDecodeError> for DiffDecodeError {
    fn from(e: VarintDecodeError) -> Self {
        match e {
            VarintDecodeError::UnexpectedEof => Self::UnexpectedEof,
            VarintDecodeError::InvalidData => Self::InvalidData,
        }
    }
}

// https://github.com/git/git/blob/master/patch-delta.c
// https://git-scm.com/docs/pack-format#_deltified_representation

pub fn decode(src: &[u8], delta: &[u8], data: &mut Vec<u8>) -> Result<()> {
    let mut reader = Cursor::new(delta);
    let src_size = decode_varint(&mut reader)?;
    let src_size = usize::try_from(src_size).map_err(|_| DiffDecodeError::OutOfMemory)?;
    if src_size != src.len() {
        cold_path();
        return Err(DiffDecodeError::BaseSizeMismatch(src_size, src.len()));
    }
    let target_size = decode_varint(&mut reader)?;
    let target_size = usize::try_from(target_size).map_err(|_| DiffDecodeError::OutOfMemory)?;

    data.clear();
    data.try_reserve(target_size)?;

    loop {
        if data.len() == target_size {
            break;
        }

        let command = read_byte(&mut reader)?;
        if command & COPY_CMD_FLAG != 0 {
            // copy command
            let mut parameter = command;
            let offset = read_copy_parameter(&mut reader, 4, &mut parameter)? as usize;
            // the 3rd bit of the size is only emitted by version 3 of the delta format, which git doesn't use. But is has supported reading it since 2006 (https://github.com/git/git/commit/d60fc1c8649f80c006b9f493c542461e81608d4b).
            let mut size = read_copy_parameter(&mut reader, 3, &mut parameter)?;

            if size == 0 {
                size = ZERO_COPY_LEN;
            }
            let end = offset
                .checked_add(size as usize)
                .ok_or(DiffDecodeError::InvalidData)?;
            let copy_data = src.get(offset..end).ok_or(DiffDecodeError::InvalidData)?;
            try_copy_data(data, copy_data, target_size)?;
        } else {
            // insert command

            if command == 0 {
                cold_path();
                return Err(DiffDecodeError::InvalidData);
            }

            let cursor_pos =
                usize::try_from(reader.position()).map_err(|_| DiffDecodeError::InvalidData)?;
            let copy_data = reader
                .get_ref()
                .get(cursor_pos..cursor_pos + command as usize)
                .ok_or(DiffDecodeError::InvalidData)?;
            try_copy_data(data, copy_data, target_size)?;
            reader.set_position(cursor_pos as u64 + command as u64);
        }
    }
    debug_assert!(data.len() <= target_size);
    if reader.position() != reader.get_ref().len() as u64 {
        cold_path();
        return Err(DiffDecodeError::InvalidData);
    }
    if data.len() != target_size {
        cold_path();
        return Err(DiffDecodeError::ResultSizeMismatch(data.len(), target_size));
    }
    Ok(())
}

fn try_copy_data(buffer: &mut Vec<u8>, copy_data: &[u8], result_size: usize) -> Result<()> {
    if buffer.len() + copy_data.len() > result_size {
        cold_path();
        return Err(DiffDecodeError::InvalidData);
    }
    debug_assert!(buffer.spare_capacity_mut().len() >= copy_data.len());
    buffer.extend_from_slice(copy_data);
    Ok(())
}

const fn read_byte(reader: &mut Cursor<&[u8]>) -> Result<u8> {
    let pos = reader.position();
    let bytes = reader.get_ref();
    if pos < bytes.len() as u64 {
        let byte = bytes[pos as usize];
        reader.set_position(pos + 1);
        return Ok(byte);
    }
    cold_path();
    Err(DiffDecodeError::UnexpectedEof)
}

#[inline]
fn read_copy_parameter(reader: &mut Cursor<&[u8]>, bits: u8, command: &mut u8) -> Result<u32> {
    assert!(bits > 0);
    assert!(bits < 5);
    let mut value = 0u32;
    for idx in 0..bits {
        if *command & 1 != 0 {
            let byte = read_byte(reader)?;
            value |= (byte as u32) << (idx * 8);
        }
        *command >>= 1;
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzz_1() {
        let base = [].as_ref();
        let delta = [0, 235, 235, 210, 210, 143, 0].as_ref();
        let mut output = vec![];
        let _r = decode(base, delta, &mut output);
    }
}

#[cfg(kani)]
mod proofs {
    use super::*;

    fn decode_varint_stub(cur: &mut Cursor<&[u8]>) -> std::result::Result<u64, VarintDecodeError> {
        let adv: u64 = kani::any_where(|x| *x <= 10);
        if adv == 0 {
            return Err(VarintDecodeError::InvalidData);
        }
        cur.set_position(cur.position() + adv);
        Ok(kani::any())
    }

    #[kani::proof]
    #[kani::unwind(5)]
    #[kani::stub(decode_varint, decode_varint_stub)]
    fn check_decode_diff() {
        let base: Vec<u8> = kani::bounded_any::<_, 32>();
        let base = base.as_ref();

        let delta: Vec<u8> = kani::bounded_any::<_, 8>();
        let delta = delta.as_ref();

        let mut output = vec![];

        let _ = decode(base, delta, &mut output);
    }

    #[kani::proof]
    #[kani::unwind(9)]
    fn check_read_copy_parameter() {
        let data: Vec<u8> = kani::bounded_any::<_, 10>();
        let data = data.as_ref();
        let mut cur = Cursor::new(data);
        cur.set_position(kani::any());
        let bits: u8 = kani::any_where(|b| *b > 0 && *b < 5);
        let mut cmd = kani::any();
        _ = read_copy_parameter(&mut cur, bits, &mut cmd);
    }
}
