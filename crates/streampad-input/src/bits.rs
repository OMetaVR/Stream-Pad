//! Bitmaps in the evdev layout (bit `n` is bit `n % 8` of byte `n / 8`), sized for any event type.

use crate::sys::KEY_MAX;
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Bits(Vec<u8>);

/// Large enough for every event type (`KEY_MAX` is the biggest code space).
const BYTES: usize = KEY_MAX as usize / 8 + 1;

impl Bits {
    pub fn new() -> Bits {
        Bits(vec![0; BYTES])
    }

    pub fn contains(&self, bit: u16) -> bool {
        self.0.get(usize::from(bit / 8)).is_some_and(|byte| byte & (1 << (bit % 8)) != 0)
    }

    pub fn insert(&mut self, bit: u16) {
        if let Some(byte) = self.0.get_mut(usize::from(bit / 8)) {
            *byte |= 1 << (bit % 8);
        }
    }

    pub fn remove(&mut self, bit: u16) {
        if let Some(byte) = self.0.get_mut(usize::from(bit / 8)) {
            *byte &= !(1 << (bit % 8));
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }

    pub fn iter(&self) -> impl Iterator<Item = u16> + '_ {
        (0..=KEY_MAX).filter(|bit| self.contains(*bit))
    }

    /// Raw buffer for evdev queries to fill.
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl Default for Bits {
    fn default() -> Self {
        Bits::new()
    }
}

impl FromIterator<u16> for Bits {
    fn from_iter<I: IntoIterator<Item = u16>>(iter: I) -> Self {
        let mut bits = Bits::new();
        iter.into_iter().for_each(|bit| bits.insert(bit));
        bits
    }
}

impl fmt::Debug for Bits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_remove_iterate() {
        let mut bits: Bits = [0, 7, 8, 96, KEY_MAX].into_iter().collect();
        assert!(bits.contains(7) && bits.contains(8) && bits.contains(KEY_MAX));
        assert!(!bits.contains(1) && !bits.contains(KEY_MAX + 1));
        bits.remove(8);
        assert_eq!(bits.iter().collect::<Vec<_>>(), vec![0, 7, 96, KEY_MAX]);
        bits.insert(KEY_MAX + 1);
        assert!(!bits.contains(KEY_MAX + 1));
        assert!(!bits.is_empty() && Bits::new().is_empty());
    }
}
