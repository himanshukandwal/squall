//! Redis-style glob matching over bytes (`stringmatchlen` semantics).
//!
//! Supports `*`, `?`, `[abc]`, `[a-z]`, `[^a]` and `\` escapes. Odd patterns
//! never fail: an unterminated `[` class ends at the end of the pattern and a
//! trailing `\` matches a literal backslash. Matching uses a single
//! backtrack point for the most recent `*`, so it runs in O(pattern * input)
//! time rather than exponentially.

/// Returns whether `text` matches the glob `pattern`.
pub(crate) fn glob_match(pattern: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0, 0);
    // Pattern index just after the last `*`, and the text index it resumes at.
    let mut star: Option<(usize, usize)> = None;

    loop {
        if p < pattern.len() && pattern[p] == b'*' {
            while p < pattern.len() && pattern[p] == b'*' {
                p += 1;
            }
            if p == pattern.len() {
                return true;
            }
            star = Some((p, t));
            continue;
        }
        if p == pattern.len() && t == text.len() {
            return true;
        }
        if p < pattern.len() && t < text.len() {
            let (ok, next) = match_one(pattern, p, text[t]);
            if ok {
                p = next;
                t += 1;
                continue;
            }
        }
        // Mismatch: let the last `*` swallow one more byte, or give up.
        match star {
            Some((sp, st)) if st < text.len() => {
                star = Some((sp, st + 1));
                p = sp;
                t = st + 1;
            }
            _ => return false,
        }
    }
}

/// Matches the single non-`*` pattern element at `p` against byte `c`.
/// Returns whether it matched and the index of the next pattern element.
fn match_one(pattern: &[u8], p: usize, c: u8) -> (bool, usize) {
    match pattern[p] {
        b'?' => (true, p + 1),
        b'[' => match_class(pattern, p + 1, c),
        b'\\' if p + 1 < pattern.len() => (pattern[p + 1] == c, p + 2),
        lit => (lit == c, p + 1),
    }
}

/// Matches a bracket class whose contents start at `p` (just after `[`).
fn match_class(pattern: &[u8], mut p: usize, c: u8) -> (bool, usize) {
    let negate = p < pattern.len() && pattern[p] == b'^';
    if negate {
        p += 1;
    }
    let mut matched = false;
    while p < pattern.len() {
        match pattern[p] {
            b']' => {
                p += 1;
                break;
            }
            b'\\' if p + 1 < pattern.len() => {
                matched |= pattern[p + 1] == c;
                p += 2;
            }
            lo if p + 2 < pattern.len() && pattern[p + 1] == b'-' => {
                let hi = pattern[p + 2];
                let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
                matched |= (lo..=hi).contains(&c);
                p += 3;
            }
            lit => {
                matched |= lit == c;
                p += 1;
            }
        }
    }
    (matched != negate, p)
}
