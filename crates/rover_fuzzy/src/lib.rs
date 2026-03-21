use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

#[derive(Clone, Debug)]
pub struct StringMatchCandidate {
    pub id: usize,
    pub string: String,
}

impl StringMatchCandidate {
    pub fn new(id: usize, string: impl Into<String>) -> Self {
        Self {
            id,
            string: string.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StringMatch {
    pub candidate_id: usize,
    pub score: i64,
    pub positions: Vec<usize>,
    pub string: String,
}

impl PartialEq for StringMatch {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score && self.candidate_id == other.candidate_id
    }
}

impl Eq for StringMatch {}

impl PartialOrd for StringMatch {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StringMatch {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score
            .cmp(&other.score)
            .reverse()
            .then_with(|| self.candidate_id.cmp(&other.candidate_id))
    }
}

pub fn match_strings(
    candidates: &[StringMatchCandidate],
    query: &str,
    max_results: usize,
) -> Vec<StringMatch> {
    if query.is_empty() {
        return candidates
            .iter()
            .take(max_results)
            .map(|c| StringMatch {
                candidate_id: c.id,
                score: 0,
                positions: Vec::new(),
                string: c.string.clone(),
            })
            .collect();
    }

    let matcher = SkimMatcherV2::default();
    let mut matches: Vec<StringMatch> = candidates
        .iter()
        .filter_map(|candidate| {
            let (score, indices) = matcher.fuzzy_indices(&candidate.string, query)?;
            Some(StringMatch {
                candidate_id: candidate.id,
                score,
                positions: indices,
                string: candidate.string.clone(),
            })
        })
        .collect();

    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches.truncate(max_results);
    matches
}

/// Normalize a query string: collapse whitespace, collapse `::`.
#[must_use]
pub fn normalize_query(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_char = None;

    for ch in input.trim().chars() {
        match (last_char, ch) {
            (Some(':'), ':') => continue,
            (Some(prev), c) if prev.is_whitespace() && c.is_whitespace() => continue,
            _ => {
                last_char = Some(ch);
            }
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(names: &[&str]) -> Vec<StringMatchCandidate> {
        names
            .iter()
            .enumerate()
            .map(|(i, n)| StringMatchCandidate::new(i, *n))
            .collect()
    }

    #[test]
    fn empty_query_returns_all_up_to_max() {
        let c = candidates(&["Alpha", "Beta", "Gamma"]);
        let results = match_strings(&c, "", 10);
        assert_eq!(results.len(), 3);
        // All scores should be 0 for empty query
        assert!(results.iter().all(|r| r.score == 0));
    }

    #[test]
    fn empty_query_respects_max_results() {
        let c = candidates(&["A", "B", "C", "D"]);
        let results = match_strings(&c, "", 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn exact_match_ranks_highest() {
        let c = candidates(&["Safari", "Settings", "Slack", "Saf"]);
        let results = match_strings(&c, "Saf", 10);
        assert!(!results.is_empty());
        // "Saf" exact match should rank first or second (tied with Safari prefix)
        let top_strings: Vec<&str> = results.iter().take(2).map(|r| r.string.as_str()).collect();
        assert!(top_strings.contains(&"Saf"));
    }

    #[test]
    fn fuzzy_match_finds_partial() {
        let c = candidates(&["Terminal", "TextEdit", "Finder"]);
        let results = match_strings(&c, "trm", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].string, "Terminal");
    }

    #[test]
    fn no_match_returns_empty() {
        let c = candidates(&["Alpha", "Beta", "Gamma"]);
        let results = match_strings(&c, "zzzzz", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn results_sorted_by_score_descending() {
        let c = candidates(&["Safari", "System Preferences", "Slack", "Saf"]);
        let results = match_strings(&c, "sa", 10);
        for w in results.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize_query("  hello   world  "), "hello world");
    }

    #[test]
    fn normalize_collapses_colons() {
        assert_eq!(normalize_query("std::io::Read"), "std:io:Read");
    }

    #[test]
    fn normalize_empty() {
        assert_eq!(normalize_query(""), "");
        assert_eq!(normalize_query("   "), "");
    }

    #[test]
    fn string_match_ordering() {
        let a = StringMatch { candidate_id: 0, score: 100, positions: vec![], string: "A".into() };
        let b = StringMatch { candidate_id: 1, score: 50, positions: vec![], string: "B".into() };
        let c = StringMatch { candidate_id: 2, score: 100, positions: vec![], string: "C".into() };
        let mut v = vec![b.clone(), c.clone(), a.clone()];
        v.sort();
        // Higher score first (reversed), then by candidate_id
        assert_eq!(v[0].string, "A");
        assert_eq!(v[1].string, "C");
        assert_eq!(v[2].string, "B");
    }
}
