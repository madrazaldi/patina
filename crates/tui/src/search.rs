//! Fuzzy matching for column filters.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

pub struct Fuzzy {
    matcher: Matcher,
    buf: Vec<char>,
}

impl Default for Fuzzy {
    fn default() -> Self {
        Self { matcher: Matcher::new(Config::DEFAULT), buf: Vec::new() }
    }
}

impl Fuzzy {
    /// Indices of matching haystacks, best first; ties keep input order.
    pub fn rank<'a, I>(&mut self, query: &str, haystacks: I) -> Vec<(usize, u32)>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut out: Vec<(usize, u32)> = haystacks
            .into_iter()
            .enumerate()
            .filter_map(|(i, h)| pattern.score(Utf32Str::new(h, &mut self.buf), &mut self.matcher).map(|s| (i, s)))
            .collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_better_matches_first() {
        let mut f = Fuzzy::default();
        let hay = ["The Matrix", "Mad Max", "Matrix Reloaded", "Amadeus"];
        let ranked = f.rank("matrix", hay);
        let names: Vec<_> = ranked.iter().map(|(i, _)| hay[*i]).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"The Matrix"));
        assert!(names.contains(&"Matrix Reloaded"));
    }
}
