use std::{
    cmp::Ordering,
    fmt,
    ops::{Add, Range, RangeInclusive, Sub},
};

use num_traits::One;
use smallvec::SmallVec;

use crate::{
    bit_stream::{ReadSafe, WriteSafe},
    BitStreamRead, BitStreamWrite,
};

/// Set of number stored as a set of non-adjacent, non-overlapping ranges
///
/// For a set with few gaps, this uses much less memory than a standard `BTreeSet`
///
/// ## Types
///
/// `N` is the number of continuous ranges that can be stored without
/// needing to allocate space on the heap
///
/// `I` is the integer used for the elements
#[derive(PartialEq, Eq)]
pub struct RangeList<const N: usize, I> {
    inner: SmallVec<[Range<I>; N]>,
}

impl<const N: usize, I: Copy + fmt::Debug> fmt::Debug for RangeList<N, I>
where
    I: Add<Output = I> + One + PartialEq,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        for e in &self.inner {
            if e.start + I::one() == e.end {
                list.entry(&e.start);
            } else {
                list.entry(e);
            }
        }
        list.finish()
    }
}

impl<const N: usize, I> Default for RangeList<N, I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, I> RangeList<N, I> {
    /// Create a new sorted set
    pub fn new() -> Self {
        Self {
            inner: SmallVec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl<const N: usize, I> RangeList<N, I>
where
    I: PartialOrd + Add<Output = I> + One + Copy,
{
    /// Find the insertion spot
    fn find(&self, v: I) -> Result<usize, usize> {
        self.inner.binary_search_by(|x| {
            if v < x.start {
                Ordering::Greater
            } else if v >= x.end {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        })
    }

    /// Check whether the inversion list contains this value
    pub fn contains(&self, v: I) -> bool {
        self.find(v).is_ok()
    }

    /// Insert a new element
    pub fn insert(&mut self, v: I) {
        let v1 = v + I::one();
        if let Some(at) = self.find(v).err() {
            if at == 0 {
                match self.inner.first_mut() {
                    Some(first) if v1 == first.start => first.start = v,
                    _ => self.inner.insert(0, v..v1),
                }
            } else if at == self.inner.len() {
                let last = unsafe { self.inner.get_unchecked_mut(at - 1) };
                if last.end == v {
                    last.end = v + I::one();
                } else {
                    self.inner.push(v..v1);
                }
            } else {
                let (left, right) = self.inner.split_at_mut(at);
                let before = unsafe { left.get_unchecked_mut(at - 1) };
                let after = unsafe { right.get_unchecked_mut(0) };

                if before.end == v {
                    if after.start == v1 {
                        before.end = after.end;
                        self.inner.remove(at);
                    } else {
                        before.end = v;
                    }
                } else if v1 == after.start {
                    after.start = v;
                } else {
                    self.inner.insert(at, v..v1);
                }
            }
        }
    }

    /// Insert multiple elements from an iterator
    pub fn insert_all<It: IntoIterator<Item = I>>(&mut self, items: It) {
        let iter = items.into_iter();
        iter.for_each(|x| self.insert(x));
    }

    /// Clear all elements
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Return an iterator over all *inclusive* ranges
    pub fn min_max_ranges(&self) -> impl Iterator<Item = RangeInclusive<I>> + '_
    where
        I: Sub<Output = I>,
    {
        self.inner.iter().map(|r| r.start..=r.end - I::one())
    }

    pub fn serialize(&mut self, bs: &mut BitStreamWrite)
    where
        I: Sub<Output = I> + WriteSafe,
    {
        bs.write_compressed(self.inner.len() as u16);
        for range in self.min_max_ranges() {
            let min_is_max = range.start() == range.end();
            bs.write_bool(min_is_max);
            bs.write(*range.start());
            if !min_is_max {
                bs.write(*range.end());
            }
        }
    }

    pub fn deserialize(bs: &mut BitStreamRead) -> crate::Result<Self>
    where
        I: ReadSafe,
    {
        let mut list = Self::new();
        for _ in 0..bs.read_compressed::<u16>()? {
            let min_is_max = bs.read_bool()?;
            let min: I = bs.read()?;
            let max = match min_is_max {
                false => bs.read()?,
                true => min,
            };
            // FIXME: this may produce broken lists
            let end = max + I::one();
            list.inner.push(min..end);
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use crate::BitStreamWrite;

    use super::RangeList;

    #[test]
    fn size() {
        assert_eq!(size_of::<RangeList<1, u32>>(), 24);
    }

    #[test]
    fn serialize() {
        let mut list: RangeList<1, u32> = Default::default();
        let mut bs = BitStreamWrite::new();
        list.serialize(&mut bs)
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)]
    fn insert() {
        let mut list: RangeList<1, i32> = Default::default();
        list.insert(1);
        assert_eq!(&list.inner[..], &[1..2]);
        assert_eq!(list.find(2), Err(1));
        list.insert(2);
        assert_eq!(&list.inner[..], vec![1..3]);
        assert_eq!(list.find(5), Err(1));
        list.insert(5);
        assert_eq!(&list.inner[..], &[1..3, 5..6]);
        list.insert(6);
        assert_eq!(&list.inner[..], &[1..3, 5..7]);
        list.insert(8);
        assert_eq!(&list.inner[..], &[1..3, 5..7, 8..9]);
        list.insert(10);
        assert_eq!(&list.inner[..], &[1..3, 5..7, 8..9, 10..11]);
        list.insert(4);
        assert_eq!(&list.inner[..], &[1..3, 4..7, 8..9, 10..11]);
        list.insert(0);
        assert_eq!(&list.inner[..], &[0..3, 4..7, 8..9, 10..11]);
        list.insert(3);
        assert_eq!(&list.inner[..], &[0..7, 8..9, 10..11]);
        list.insert(9);
        assert_eq!(&list.inner[..], &[0..7, 8..11]);
        list.insert(9);
        assert_eq!(&list.inner[..], &[0..7, 8..11]);
        list.insert(7);
        assert_eq!(&list.inner[..], &[0..11]);
    }
}
