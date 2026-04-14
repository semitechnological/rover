//! Emoji picker plugin — provides quick emoji search via shortcodes like `:sob:`
//! and inserts the selected emoji via clipboard.
//!
//! Uses fuzzy matching on emoji shortcodes, names, and common variations.
//! No hardcoded alias tables — all matching is done via fuzzy string matching.

use emojis::{iter as emoji_iter, Emoji};
use rover_fuzzy::{match_strings, StringMatch, StringMatchCandidate};
use rover_plugin_api::*;

#[derive(Clone)]
struct EmojiEntry {
    shortcode: String,
    emoji_char: String,
    name: String,
    search_variants: Vec<String>,
}

pub struct EmojiPlugin {
    entries: Vec<EmojiEntry>,
    candidates: Vec<StringMatchCandidate>,
    matches: Vec<(StringMatch, usize)>,
}

impl EmojiPlugin {
    pub fn new() -> Self {
        let mut entries = Vec::new();

        for emoji in emoji_iter() {
            if let Some(shortcode) = emoji.shortcode() {
                let shortcode = shortcode.to_string();
                let name = emoji.name().to_lowercase();

                let mut search_variants = vec![shortcode.clone(), name.clone()];

                let name_clean = name.replace("_", " ").replace("-", " ");
                if name_clean != name {
                    search_variants.push(name_clean);
                }

                let first_word = name.split_whitespace().next().unwrap_or(&name);
                if first_word != name {
                    search_variants.push(first_word.to_string());
                }

                entries.push(EmojiEntry {
                    shortcode,
                    emoji_char: emoji.as_str().to_string(),
                    name: emoji.name().to_string(),
                    search_variants,
                });
            }
        }

        let candidates: Vec<_> = entries
            .iter()
            .enumerate()
            .flat_map(|(i, e)| {
                e.search_variants
                    .iter()
                    .map(move |v| StringMatchCandidate::new(i, v.as_str()))
            })
            .collect();

        log::info!(
            "EmojiPlugin: indexed {} emojis with {} search variants",
            entries.len(),
            candidates.len()
        );

        Self {
            entries,
            candidates,
            matches: Vec::new(),
        }
    }
}

impl Default for EmojiPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EmojiPlugin {
    fn id(&self) -> &str {
        "builtin.emoji"
    }
    fn name(&self) -> &str {
        "Emoji Picker"
    }
    fn description(&self) -> &str {
        "Search and insert emojis by shortcode"
    }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        log::info!("EmojiPlugin: loaded {} emojis", self.entries.len());
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn priority(&self) -> i32 {
        20
    }

    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor {
            id: "emoji".to_string(),
            name: "Emoji".to_string(),
            description: "Search and insert emoji".to_string(),
            icon: Some(IconHint::Emoji("😀".to_string())),
            keywords: vec!["emoji".to_string(), "smiley".to_string()],
            has_submenu: false,
        }]
    }

    fn on_query(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        let trimmed = query.trim().trim_start_matches(':').trim_end_matches(':');

        if trimmed.is_empty() {
            self.matches.clear();
            return Vec::new();
        }

        let fuzzy = match_strings(&self.candidates, trimmed, 50);

        let mut seen_entries = std::collections::HashSet::new();
        self.matches = fuzzy
            .into_iter()
            .filter(|m| {
                if seen_entries.insert(m.candidate_id) {
                    true
                } else {
                    false
                }
            })
            .map(|m| (m.clone(), m.candidate_id))
            .collect();

        self.matches
            .iter()
            .filter_map(|(m, _idx)| {
                self.entries.get(m.candidate_id).map(|entry| {
                    let emoji_str = &entry.emoji_char;
                    MatchResult {
                        id: format!("emoji:{}", entry.shortcode),
                        title: format!("{} :{}:", emoji_str, entry.shortcode),
                        subtitle: Some(entry.name.clone()),
                        icon: Some(IconHint::Emoji(emoji_str.to_string())),
                        score: m.score as f64,
                        plugin_id: self.id().to_string(),
                    }
                })
            })
            .collect()
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if let Some(shortcode) = match_id.strip_prefix("emoji:") {
            if let Some(entry) = self.entries.iter().find(|e| e.shortcode == shortcode) {
                let emoji_str = entry.emoji_char.clone();
                let emoji_clone = emoji_str.clone();
                std::thread::spawn(move || {
                    if let Some(mut cb) = arboard::Clipboard::new().ok() {
                        let _ = cb.set_text(&emoji_clone);
                    }
                });
                log::info!("EmojiPlugin: copied {} to clipboard", emoji_str);
                return ConfirmResult::Dismiss;
            }
        }
        ConfirmResult::Stay
    }

    fn actions_for_match(&self, match_id: &str) -> Vec<ActionDescriptor> {
        if match_id.starts_with("emoji:") {
            vec![ActionDescriptor {
                id: "copy".to_string(),
                name: "Copy to clipboard".to_string(),
                shortcut: Some("Enter".to_string()),
            }]
        } else {
            vec![]
        }
    }
}
