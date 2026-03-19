use gpui::prelude::*;
use gpui::*;
use rover_commands::{calculator, Command, CommandRegistry};
use rover_fuzzy::{match_strings, normalize_query, StringMatch, StringMatchCandidate};
use rover_picker::{Picker, PickerDelegate};
use rover_theme::theme;
use std::sync::Arc;

pub struct RoverDelegate {
    all_commands: Vec<Command>,
    matches: Vec<StringMatch>,
    selected_ix: usize,
    query: String,
    calculator_result: Option<String>,
}

impl RoverDelegate {
    pub fn new(commands: Vec<Command>) -> Self {
        let candidates: Vec<StringMatchCandidate> = commands
            .iter()
            .enumerate()
            .map(|(i, c)| StringMatchCandidate::new(i, &c.name))
            .collect();
        let matches = match_strings(&candidates, "", 50);
        Self {
            all_commands: commands,
            matches,
            selected_ix: 0,
            query: String::new(),
            calculator_result: None,
        }
    }

    fn render_calculator_row(
        &self,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let result = self.calculator_result.as_ref().unwrap();
        let mut row = div().mx_1().px_2().py(px(6.0)).w_full().rounded_md();
        if selected {
            row = row.bg(colors.element_selected);
        }
        row.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .text_color(colors.text_muted)
                        .text_size(px(14.0))
                        .child(SharedString::from("=")),
                )
                .child(
                    div()
                        .text_color(colors.text_accent)
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(SharedString::from(result.clone())),
                ),
        )
        .into_any_element()
    }

    fn render_command_row(
        &self,
        command: &Command,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let mut row = div().mx_1().px_2().py(px(6.0)).w_full().rounded_md();
        if selected {
            row = row.bg(colors.element_selected);
        }
        row.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_color(colors.text)
                        .text_size(px(14.0))
                        .child(SharedString::from(command.name.clone())),
                )
                .child(
                    div()
                        .text_color(colors.text_muted)
                        .text_size(px(12.0))
                        .child(SharedString::from(command.description.clone())),
                ),
        )
        .into_any_element()
    }
}

impl PickerDelegate for RoverDelegate {
    type ListItem = AnyElement;

    fn match_count(&self) -> usize {
        let extra = if self.calculator_result.is_some() {
            1
        } else {
            0
        };
        self.matches.len() + extra
    }

    fn selected_index(&self) -> usize {
        self.selected_ix
    }

    fn set_selected_index(&mut self, ix: usize, _cx: &mut Context<Picker<Self>>) {
        self.selected_ix = ix;
    }

    fn placeholder_text(&self) -> Arc<str> {
        Arc::from("Search apps, commands, or type math...")
    }

    fn update_matches(&mut self, query: String, _cx: &mut Context<Picker<Self>>) -> Task<()> {
        let normalized = normalize_query(&query);
        self.query = query;

        self.calculator_result = calculator::evaluate(&self.query).map(|v| {
            if v.fract() == 0.0 {
                format!("{}", v as i64)
            } else {
                format!("{:.6}", v)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            }
        });

        let candidates: Vec<StringMatchCandidate> = self
            .all_commands
            .iter()
            .enumerate()
            .map(|(i, c)| StringMatchCandidate::new(i, &c.name))
            .collect();

        self.matches = match_strings(&candidates, &normalized, 50);
        self.selected_ix = 0;
        Task::ready(())
    }

    fn confirm(&mut self, _cx: &mut Context<Picker<Self>>) -> bool {
        if self.calculator_result.is_some() && self.selected_ix == 0 {
            // TODO: copy calc result to clipboard
            return false;
        }

        let match_ix = if self.calculator_result.is_some() {
            self.selected_ix - 1
        } else {
            self.selected_ix
        };

        if let Some(string_match) = self.matches.get(match_ix) {
            if let Some(command) = self.all_commands.get(string_match.candidate_id) {
                CommandRegistry::execute(command);
                return true;
            }
        }
        false
    }

    fn dismissed(&mut self, _cx: &mut Context<Picker<Self>>) {}

    fn render_match(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem> {
        let theme = theme(cx);
        let colors = &theme.colors;

        if self.calculator_result.is_some() && ix == 0 {
            return Some(self.render_calculator_row(selected, colors));
        }

        let match_ix = if self.calculator_result.is_some() {
            ix - 1
        } else {
            ix
        };
        let string_match = self.matches.get(match_ix)?;
        let command = self.all_commands.get(string_match.candidate_id)?;
        Some(self.render_command_row(command, selected, colors))
    }
}
