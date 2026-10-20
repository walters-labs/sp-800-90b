// Prefix-doubling suffix array with two stable counting-sort passes per round.
// Rank zero represents the missing second half. O(n log n) time, O(n) storage.
fn suffix_array(s: &[u8]) -> Vec<usize> {
    let n = s.len();
    let mut sa: Vec<_> = (0..n).collect();
    let mut tmp = vec![0; n];
    let mut rank: Vec<_> = s.iter().map(|&x| usize::from(x) + 1).collect();
    let mut next = vec![0; n];
    let mut counts = vec![0; n.max(256) + 1];
    let mut classes = 256;
    let mut width = 1;
    while width < n {
        for second in [true, false] {
            counts[..=classes].fill(0);
            let key = |i: usize| {
                if second {
                    if i < n - width { rank[i + width] } else { 0 }
                } else {
                    rank[i]
                }
            };
            for &i in &sa {
                counts[key(i)] += 1;
            }
            let mut sum = 0;
            for count in &mut counts[..=classes] {
                let value = *count;
                *count = sum;
                sum += value;
            }
            for &i in &sa {
                let k = key(i);
                tmp[counts[k]] = i;
                counts[k] += 1;
            }
            std::mem::swap(&mut sa, &mut tmp);
        }
        let mut new_classes = 1;
        next[sa[0]] = 1;
        let second = |i: usize| if i < n - width { rank[i + width] } else { 0 };
        for pair in sa.windows(2) {
            if (rank[pair[0]], second(pair[0])) != (rank[pair[1]], second(pair[1])) {
                new_classes += 1;
            }
            next[pair[1]] = new_classes;
        }
        std::mem::swap(&mut rank, &mut next);
        classes = new_classes;
        if classes == n || width >= n - width {
            break;
        }
        width *= 2;
    }
    sa
}

// Kasai LCP followed by a monotone histogram stack. A run of k adjacent
// LCP entries at least h represents k+1 occurrences of the same h-tuple.
pub(crate) fn tuple_counts(s: &[u8], include_pairs: bool) -> (Vec<usize>, Vec<u128>) {
    let n = s.len();
    let sa = suffix_array(s);
    let mut inverse = vec![0; n];
    for (r, &i) in sa.iter().enumerate() {
        inverse[i] = r;
    }
    let mut lcp = vec![0; n];
    let mut matched = 0;
    for i in 0..n {
        let r = inverse[i];
        if r == 0 {
            matched = 0;
            continue;
        }
        let j = sa[r - 1];
        while matched < n - i && matched < n - j && s[i + matched] == s[j + matched] {
            matched += 1;
        }
        lcp[r] = matched;
        matched = matched.saturating_sub(1);
    }
    drop(sa);
    drop(inverse);
    let mut q = vec![1; n + 1];
    let mut events = if include_pairs {
        vec![0i128; n + 2]
    } else {
        Vec::new()
    };
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for (i, height) in lcp.into_iter().chain(std::iter::once(0)).enumerate() {
        let mut start = i;
        while let Some(&(left, h)) = stack.last() {
            if h <= height {
                break;
            }
            stack.pop();
            let occurrences = i - left + 1;
            q[h] = q[h].max(occurrences);
            if include_pairs {
                let lower = height.max(stack.last().map_or(0, |&(_, level)| level));
                // A suffix-tree interval contributes choose(occurrences,2)
                // at depths lower+1..=h. Overlapping positions are included.
                let count = occurrences as i128;
                let pairs = if count % 2 == 0 {
                    (count / 2) * (count - 1)
                } else {
                    count * ((count - 1) / 2)
                };
                events[lower + 1] += pairs;
                events[h + 1] -= pairs;
            }
            start = left;
        }
        if height > 0 && stack.last().is_none_or(|&(_, h)| h < height) {
            stack.push((start, height));
        }
    }
    for h in (1..n).rev() {
        q[h] = q[h].max(q[h + 1]);
    }
    let mut pairs = Vec::new();
    if include_pairs {
        let mut total = 0i128;
        pairs = events
            .into_iter()
            .take(n + 1)
            .map(|delta| {
                total += delta;
                debug_assert!(total >= 0);
                total as u128
            })
            .collect();
    }
    (q, pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    fn check(s: &[u8]) {
        let (_, pairs) = tuple_counts(s, true);
        for (width, &actual) in pairs.iter().enumerate().skip(1) {
            let mut counts = HashMap::new();
            for tuple in s.windows(width) {
                *counts.entry(tuple).or_insert(0u128) += 1;
            }
            let expected: u128 = counts.values().map(|&c| c * (c - 1) / 2).sum();
            assert_eq!(actual, expected, "{s:?}, width={width}");
        }
    }
    #[test]
    fn pair_counts_match_exhaustive_counts() {
        for n in 2..=9 {
            for pattern in 0..1usize << n {
                check(
                    &(0..n)
                        .map(|i| ((pattern >> i) & 1) as u8)
                        .collect::<Vec<_>>(),
                );
            }
        }
        check(&(0..=255).cycle().take(600).collect::<Vec<_>>());
    }
}
