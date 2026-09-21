//! Local selection/filter state. Only committed choices belong in persisted app state.
use crate::{clean, theme::Theme};
use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};

#[derive(Clone, Debug, Default)]
pub(crate) struct Selection {
    query: String,
    cursor: usize,
}

fn filtered(options: &[String], query: &str) -> Vec<usize> {
    let query = query.to_lowercase();
    let mut matches: Vec<_> = options
        .iter()
        .enumerate()
        .filter_map(|(index, option)| {
            let option = option.to_lowercase();
            let score = if query.is_empty() || option.starts_with(&query) {
                0
            } else if let Some(position) = option.find(&query) {
                100 + position
            } else {
                let mut wanted = query.chars().peekable();
                let mut gaps = 0;
                for ch in option.chars() {
                    if wanted.peek() == Some(&ch) {
                        wanted.next();
                    } else {
                        gaps += 1;
                    }
                }
                if wanted.peek().is_some() {
                    return None;
                }
                1000 + gaps
            };
            Some((score, index))
        })
        .collect();
    matches.sort_unstable();
    matches.into_iter().map(|(_, index)| index).collect()
}

impl Selection {
    pub fn key(
        &mut self,
        options: &[String],
        selected: &[String],
        searchable: bool,
        multiple: bool,
        key: KeyCode,
    ) -> (bool, Option<Vec<String>>) {
        use KeyCode::*;
        let matches = filtered(options, &self.query);
        self.cursor = self.cursor.min(matches.len().saturating_sub(1));
        match key {
            Up => self.cursor = self.cursor.saturating_sub(1),
            Down => self.cursor = (self.cursor + 1).min(matches.len().saturating_sub(1)),
            Home => self.cursor = 0,
            End => self.cursor = matches.len().saturating_sub(1),
            Enter if multiple => {}
            Enter | Char(' ') if multiple || key == Enter => {
                if let Some(index) = matches.get(self.cursor) {
                    let chosen = &options[*index];
                    let next = if multiple {
                        options
                            .iter()
                            .filter(|option| {
                                (selected.contains(option) && *option != chosen)
                                    || (*option == chosen && !selected.contains(option))
                            })
                            .cloned()
                            .collect()
                    } else {
                        vec![chosen.clone()]
                    };
                    return (true, Some(next));
                }
            }
            Backspace if searchable => {
                self.query.pop();
                self.cursor = 0;
            }
            Char(ch) if searchable && !ch.is_control() => {
                if self.query.chars().count() < 80 {
                    self.query.push(ch);
                    self.cursor = 0;
                }
            }
            _ => return (false, None),
        }
        (true, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        theme: &Theme,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        options: &[String],
        selected: &[String],
        searchable: bool,
        multiple: bool,
        focused: bool,
        bordered: bool,
    ) {
        let border = theme
            .block(BorderType::Plain)
            .title(clean(title))
            .border_style(Style::default().fg(if focused { theme.focus } else { theme.border }));
        let border = if bordered { border } else { Block::default() };
        let mut inner = border.inner(area);
        frame.render_widget(border, area);
        if searchable {
            let text = vec![
                Line::from(format!(
                    "Search: {}{}",
                    clean(&self.query),
                    if focused { "▏" } else { "" }
                )),
                Line::from(if multiple {
                    format!("{} selected · Space toggle", selected.len())
                } else {
                    format!(
                        "Selected: {} · Enter choose",
                        clean(selected.first().map(String::as_str).unwrap_or(""))
                    )
                })
                .style(Style::default().fg(theme.muted)),
            ];
            frame.render_widget(
                Paragraph::new(text),
                Rect::new(inner.x, inner.y, inner.width, inner.height.min(2)),
            );
            let used = inner.height.min(2);
            inner.y += used;
            inner.height -= used;
        } else if multiple && bordered {
            frame.render_widget(
                Block::default().title_bottom("Space toggle · Tab next"),
                area,
            );
        }
        let matches = filtered(options, &self.query);
        if matches.is_empty() {
            frame.render_widget(
                Paragraph::new("No matches").style(Style::default().fg(theme.muted)),
                inner,
            );
            return;
        }
        let items = matches.iter().map(|index| {
            let option = &options[*index];
            let marker = if multiple {
                if selected.contains(option) {
                    "[x] "
                } else {
                    "[ ] "
                }
            } else if selected.contains(option) {
                "● "
            } else {
                "  "
            };
            ListItem::new(format!("{marker}{}", clean(option)))
        });
        let mut state =
            ListState::default().with_selected(Some(self.cursor.min(matches.len() - 1)));
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("› ")
                .highlight_style(if focused {
                    Style::default()
                        .bg(theme.primary)
                        .fg(theme.text_on(theme.primary))
                } else {
                    Style::default().bold()
                }),
            inner,
            &mut state,
        );
    }
}
