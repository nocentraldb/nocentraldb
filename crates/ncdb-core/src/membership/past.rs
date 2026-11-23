//! A node's causal past as a set of node numbers (NOC-442). Each node of a
//! [`super::input::Memo`] gets a number when it is first seen, in an order
//! where its dependencies come first, so the past of a node written after
//! all the others is one range of numbers. A space with 10,000 persons
//! held each past as a set of change ids before: the square of the
//! persons, about 2 GB. Kept as ranges, a chain of control changes holds
//! one range per node.

/// A set of node numbers as sorted ranges, each `start..end`, apart and
/// not touching: equal sets, equal ranges.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Past(Vec<(u32, u32)>);

impl Past {
    /// Whether the set holds `number`.
    pub(crate) fn contains(&self, number: u32) -> bool {
        let i = self.0.partition_point(|(_, end)| *end <= number);
        self.0.get(i).is_some_and(|(start, _)| *start <= number)
    }

    /// How many numbers the set holds.
    pub(crate) fn len(&self) -> usize {
        self.0.iter().map(|(start, end)| (end - start) as usize).sum()
    }

    /// The numbers of the set, ascending.
    pub(crate) fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().flat_map(|(start, end)| *start..*end)
    }

    /// Adds `number`.
    pub(crate) fn insert(&mut self, number: u32) {
        self.extend(&Past(vec![(number, number + 1)]));
    }

    /// Adds every number of `other`.
    pub(crate) fn extend(&mut self, other: &Past) {
        if other.0.is_empty() {
            return;
        }
        let mut ranges: Vec<(u32, u32)> = Vec::with_capacity(self.0.len() + other.0.len());
        let (mut a, mut b) = (self.0.iter().peekable(), other.0.iter().peekable());
        loop {
            let next = match (a.peek(), b.peek()) {
                (Some(x), Some(y)) if x.0 <= y.0 => a.next(),
                (Some(_), Some(_)) | (None, Some(_)) => b.next(),
                (Some(_), None) => a.next(),
                (None, None) => break,
            };
            let &(start, end) = next.expect("a range");
            match ranges.last_mut() {
                Some(last) if start <= last.1 => last.1 = last.1.max(end),
                _ => ranges.push((start, end)),
            }
        }
        self.0 = ranges;
    }
}

impl FromIterator<u32> for Past {
    fn from_iter<I: IntoIterator<Item = u32>>(numbers: I) -> Past {
        let mut numbers: Vec<u32> = numbers.into_iter().collect();
        numbers.sort_unstable();
        let mut ranges: Vec<(u32, u32)> = Vec::new();
        for n in numbers {
            match ranges.last_mut() {
                Some(last) if n <= last.1 => last.1 = last.1.max(n + 1),
                _ => ranges.push((n, n + 1)),
            }
        }
        Past(ranges)
    }
}

#[cfg(test)]
mod tests {
    // On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
    // this routes #[test] there, so every test runs on both targets (NOC-10).
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::Past;

    fn of(numbers: &[u32]) -> Past {
        let mut past = Past::default();
        for n in numbers {
            past.insert(*n);
        }
        past
    }

    #[test]
    fn a_set_holds_what_was_added_in_any_order() {
        let past = of(&[5, 1, 2, 9, 3, 0]);
        assert_eq!(past.iter().collect::<Vec<_>>(), [0, 1, 2, 3, 5, 9]);
        assert_eq!(past.len(), 6);
        for n in 0..12 {
            assert_eq!(past.contains(n), [0, 1, 2, 3, 5, 9].contains(&n), "{n}");
        }
        assert_eq!(past, of(&[0, 1, 2, 3, 5, 9, 2]), "equal sets, equal ranges");
    }

    #[test]
    fn a_chain_is_one_range() {
        let past = of(&(0..1000).collect::<Vec<_>>());
        assert_eq!(past.0, [(0, 1000)]);
    }

    #[test]
    fn extending_merges_ranges_that_overlap_or_touch() {
        let mut past = of(&[0, 1, 2, 7, 8]);
        past.extend(&of(&[3, 4, 10]));
        assert_eq!(past.0, [(0, 5), (7, 9), (10, 11)]);
        past.extend(&of(&[9]));
        assert_eq!(past.0, [(0, 5), (7, 11)]);
    }
}
