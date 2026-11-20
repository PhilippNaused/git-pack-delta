#[allow(unused_imports)]
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[allow(unused_imports)]
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

// TODO: try returning u32 on all paths

/// Counts the number of matching bytes from the start of two slices.
#[inline(always)]
pub fn count_matches(a: &[u8], b: &[u8]) -> usize {
    cfg_select! {
        target_feature = "avx512bw" => unsafe { count_matches_avx512bw(a, b) },
        target_feature = "avx2" => unsafe { count_matches_avx2(a, b) },
        target_feature = "sse2" => unsafe { count_matches_sse2(a, b) },
        _ => count_matches_u128(a, b),
    }
}

/// Fallback implementation for when the remaining bytes do not fit into a full word.
#[inline]
fn count_matches_u8(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| **x == **y).count()
}

#[inline]
pub fn count_matches_rev(a: &[u8], b: &[u8], n: usize) -> usize {
    a.iter()
        .rev()
        .zip(b.iter().rev())
        .take(n)
        .take_while(|(x, y)| **x == **y)
        .count()
}

trait NumWord: Sized {
    const SIZE: usize = core::mem::size_of::<Self>();
    unsafe fn from_ne_slice(bytes: &[u8]) -> Self;
    // both methods must be "unsafe" to use #[target_feature]
    unsafe fn count_matches(a: Self, b: Self) -> Option<usize>;
}

impl NumWord for u128 {
    // The compiler should translate this into SIMD instructions (if available)
    #[inline]
    unsafe fn from_ne_slice(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() >= Self::SIZE);
        let array = unsafe { bytes[..Self::SIZE].try_into().unwrap_unchecked() };
        Self::from_ne_bytes(array)
    }
    #[inline]
    unsafe fn count_matches(a: Self, b: Self) -> Option<usize> {
        let c = a ^ b;
        if c != 0 {
            let z = cfg_select! {
                target_endian = "little" => c.trailing_zeros(),
                _ => c.leading_zeros(),
            };
            Some({ z / 8 } as usize)
        } else {
            None
        }
    }
}

impl NumWord for __m128i {
    #[inline]
    #[target_feature(enable = "sse2")]
    unsafe fn from_ne_slice(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() >= Self::SIZE);
        #[cfg(target_feature = "sse3")]
        unsafe {
            _mm_lddqu_si128(bytes.as_ptr() as _)
        }
        #[cfg(not(target_feature = "sse3"))]
        unsafe {
            _mm_loadu_si128(bytes.as_ptr() as _)
        }
    }
    #[inline]
    #[target_feature(enable = "sse2")]
    unsafe fn count_matches(a: Self, b: Self) -> Option<usize> {
        let c = _mm_cmpeq_epi8(a, b);
        let c = _mm_movemask_epi8(c);
        if c != 0xffff {
            return Some(c.trailing_ones() as usize);
        }
        None
    }
}

impl NumWord for __m256i {
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn from_ne_slice(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() >= Self::SIZE);
        unsafe { _mm256_lddqu_si256(bytes.as_ptr() as _) }
    }
    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn count_matches(a: Self, b: Self) -> Option<usize> {
        #[cfg(all(target_feature = "avx512bw", target_feature = "avx512vl"))]
        unsafe {
            let c = _mm256_cmpneq_epu8_mask(a, b);
            if c != 0 {
                return Some(c.trailing_zeros() as usize);
            }
        }
        #[cfg(not(all(target_feature = "avx512bw", target_feature = "avx512vl")))]
        {
            let c = _mm256_cmpeq_epi8(a, b);
            let c = _mm256_movemask_epi8(c);
            if c != -1 {
                return Some(c.trailing_ones() as usize);
            }
        }
        None
    }
}

impl NumWord for __m512i {
    #[inline]
    #[target_feature(enable = "avx512bw")]
    unsafe fn from_ne_slice(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() >= Self::SIZE);
        #[cfg(target_feature = "avx512f")]
        unsafe {
            _mm512_loadu_si512(bytes.as_ptr() as _)
        }
        #[cfg(not(target_feature = "avx512f"))]
        unsafe {
            _mm512_loadu_epi8(bytes.as_ptr() as _)
        }
    }
    #[inline]
    #[target_feature(enable = "avx512bw")]
    unsafe fn count_matches(a: Self, b: Self) -> Option<usize> {
        let c = _mm512_cmpneq_epu8_mask(a, b);
        if c != 0 {
            return Some(c.trailing_zeros() as usize);
        }
        None
    }
}

#[allow(unused)]
pub fn count_matches_u128(a: &[u8], b: &[u8]) -> usize {
    count_matches_word::<u128>(a, b)
}

/// # Safety
/// UB if the CPU does not support SSE2.
#[allow(unused)]
#[target_feature(enable = "sse2")]
pub unsafe fn count_matches_sse2(a: &[u8], b: &[u8]) -> usize {
    count_matches_word::<__m128i>(a, b)
}

/// # Safety
/// UB if the CPU does not support AVX2.
#[allow(unused)]
#[target_feature(enable = "avx2")]
pub unsafe fn count_matches_avx2(a: &[u8], b: &[u8]) -> usize {
    count_matches_word::<__m256i>(a, b)
}

/// # Safety
/// UB if the CPU does not support AVX512BW.
#[allow(unused)]
#[target_feature(enable = "avx512bw")]
pub unsafe fn count_matches_avx512bw(a: &[u8], b: &[u8]) -> usize {
    count_matches_word::<__m512i>(a, b)
}

#[inline(always)]
fn count_matches_word<T: NumWord>(a: &[u8], b: &[u8]) -> usize {
    let max = a.len().min(b.len());
    let mut a = &a[..max];
    let mut b = &b[..max];
    let mut i = 0;
    while a.len() >= T::SIZE {
        unsafe {
            let a = T::from_ne_slice(&a[..T::SIZE]);
            let b = T::from_ne_slice(&b[..T::SIZE]);
            if let Some(m) = T::count_matches(a, b) {
                return i + m;
            }
        }
        a = &a[T::SIZE..];
        b = &b[T::SIZE..];
        i += T::SIZE;
    }
    i + count_matches_u8(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    // use yare::parameterized; // can't use yare here since some of the functions are conditionally compiled

    #[test]
    fn test_count_matches_u8() {
        test_count_matches(count_matches_u8);
    }

    #[test]
    fn test_count_matches_u128() {
        test_count_matches(count_matches_u128);
    }

    #[test]
    #[cfg(target_feature = "sse2")]
    fn test_count_matches_sse2() {
        test_count_matches(count_matches_sse2);
    }

    #[test]
    #[cfg(target_feature = "avx2")]
    fn test_count_matches_avx2() {
        test_count_matches(count_matches_avx2);
    }

    #[test]
    #[cfg(target_feature = "avx512bw")]
    fn test_count_matches_avx512bw() {
        test_count_matches(count_matches_avx512bw);
    }

    fn test_count_matches(count: unsafe fn(&[u8], &[u8]) -> usize) {
        unsafe {
            assert_eq!(count(b"hello", b"hello"), 5);
            assert_eq!(count(b"hello", b"world"), 0);
            assert_eq!(count(b"hello", b"help"), 3);
            assert_eq!(count(b"", b"hello"), 0);
            assert_eq!(count(b"hello", b""), 0);
        }

        for _ in 0..1000 {
            let size = rand::random_range(1..=1024);
            let mut buf = vec![0u8; size];
            rand::fill(&mut buf);
            let mut buf2 = buf.clone();
            let ixd = rand::random_range(0..buf2.len());
            buf2[ixd] ^= 0xFF; // modify one byte
            let match_len = unsafe { count(&buf, &buf2) };
            assert_eq!(match_len, ixd);
        }
    }

    #[test]
    fn test_count_matches_rev() {
        assert_eq!(count_matches_rev(b"hello", b"hello", 10), 5);
        assert_eq!(count_matches_rev(b"hello", b"world", 10), 0);
        assert_eq!(count_matches_rev(b"hello", b"jello", 10), 4);
        assert_eq!(count_matches_rev(b"hello", b"jello", 2), 2);
        assert_eq!(count_matches_rev(b"world!", b"hello, world!", 7), 6);
        assert_eq!(count_matches_rev(b"world!", b"hello, world!", 4), 4);
        assert_eq!(count_matches_rev(b"hello, world!", b"world!", 7), 6);
        assert_eq!(count_matches_rev(b"hello, world!", b"world!", 4), 4);
        assert_eq!(count_matches_rev(b"", b"hello", 10), 0);
        assert_eq!(count_matches_rev(b"hello", b"", 10), 0);

        for _ in 0..1000 {
            let size = rand::random_range(1..=1024);
            let mut buf = vec![0u8; size];
            rand::fill(&mut buf);
            let mut buf2 = buf.clone();
            let ixd = rand::random_range(0..buf2.len());
            buf2[ixd] ^= 0xFF; // modify one byte
            let match_len = count_matches_rev(&buf, &buf2, size);
            assert_eq!(match_len, size - ixd - 1);
        }
    }
}
