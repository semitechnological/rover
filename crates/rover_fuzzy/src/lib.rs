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
