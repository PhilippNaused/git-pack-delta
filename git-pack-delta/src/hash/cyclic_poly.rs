use super::RollingHash;
use core::fmt::Debug;

pub struct CyclicPoly32(cyclic_poly_23::CyclicPoly32);
pub struct CyclicPoly64(cyclic_poly_23::CyclicPoly64);

impl Debug for CyclicPoly32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("CyclicPoly32").field(&self.value()).finish()
    }
}

impl Debug for CyclicPoly64 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("CyclicPoly64").field(&self.value()).finish()
    }
}

impl RollingHash for CyclicPoly32 {
    const WINDOW: usize = 16;
    type H = u32;
    #[inline]
    fn empty() -> Self::H {
        0
    }
    #[inline]
    fn value(&self) -> Self::H {
        self.0.value()
    }
    #[inline]
    fn from_slice(data: &[u8]) -> Self {
        let mut s = cyclic_poly_23::CyclicPoly32::new(Self::WINDOW);
        s.update(data);
        Self(s)
    }
    #[inline]
    fn reset_with(&mut self, data: &[u8]) {
        self.0.reset_hash();
        self.0.update(data);
    }
    #[inline]
    fn roll_once(&mut self, new: u8, old: u8) {
        self.0.rotate(old, new);
    }
    #[inline]
    fn value_from_slice(data: &[u8]) -> Self::H {
        cyclic_poly_23::CyclicPoly32::calculate(data)
    }
    #[inline]
    fn apply_mask(hash: Self::H, mask: u32) -> usize {
        (hash & mask) as usize
    }
}

impl RollingHash for CyclicPoly64 {
    const WINDOW: usize = 16;
    type H = u64;
    #[inline]
    fn empty() -> Self::H {
        0
    }
    #[inline]
    fn value(&self) -> Self::H {
        self.0.value()
    }
    #[inline]
    fn from_slice(data: &[u8]) -> Self {
        let mut s = cyclic_poly_23::CyclicPoly64::new(Self::WINDOW);
        s.update(data);
        Self(s)
    }
    #[inline]
    fn reset_with(&mut self, data: &[u8]) {
        self.0.reset_hash();
        self.0.update(data);
    }
    #[inline]
    fn roll_once(&mut self, new: u8, old: u8) {
        self.0.rotate(old, new);
    }
    #[inline]
    fn value_from_slice(data: &[u8]) -> Self::H {
        cyclic_poly_23::CyclicPoly64::calculate(data)
    }
    #[inline]
    fn apply_mask(hash: Self::H, mask: u32) -> usize {
        (hash & mask as u64) as usize
    }
}
