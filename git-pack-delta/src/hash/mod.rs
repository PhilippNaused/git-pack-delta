mod cyclic_poly;
mod git_rabin;

pub use cyclic_poly::CyclicPoly32;
pub use cyclic_poly::CyclicPoly64;
pub use git_rabin::GitRabin;

pub trait RollingHash {
    const WINDOW: usize;
    type H: Eq + Clone + Copy + core::fmt::Debug;
    fn empty() -> Self::H;
    fn value(&self) -> Self::H;
    fn from_slice(data: &[u8]) -> Self;
    fn reset_with(&mut self, data: &[u8]);
    fn roll_once(&mut self, new: u8, old: u8);
    fn value_from_slice(data: &[u8]) -> Self::H;
    fn apply_mask(hash: Self::H, mask: u32) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DATA: &[u8] = include_bytes!("../../tests/data/diff-delta.c");

    #[test]
    fn verify_rolling_git_rabin() {
        verify_rolling::<GitRabin>();
    }

    #[test]
    fn verify_rolling_cyclic_poly_32() {
        verify_rolling::<CyclicPoly32>();
    }

    #[test]
    fn verify_rolling_cyclic_poly_64() {
        verify_rolling::<CyclicPoly64>();
    }

    fn verify_rolling<H: RollingHash>() {
        let size = H::WINDOW;
        let data = SAMPLE_DATA;
        let mut rolling = H::from_slice(&data[..size]);
        let mut i = size;
        while i < data.len() - 1 {
            let old = data[i - size];
            let new = data[i];
            let window = &data[i - size + 1..i + 1];
            rolling.roll_once(new, old);
            let new_value = rolling.value();
            let compare = H::from_slice(window).value();
            let compare2 = H::value_from_slice(window);

            assert_eq!(compare, compare2);
            assert_eq!(new_value, compare2);
            assert_eq!(new_value, compare2);
            assert_ne!(new_value, H::empty());
            i += 1;
        }
    }
}
