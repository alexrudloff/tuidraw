//! Native adaptations of tuiparts' theme, button, badge and switch recipes.
//! See THIRD_PARTY_NOTICES.md. Keypad layout is an application-owned composition.
use ratatui::{prelude::*, widgets::*};
use serde::Deserialize;

use crate::theme::Theme;

/// Modified keys keep accelerators separate from text entry and host commands.
pub fn hotkey(value: &str) -> Option<(crossterm::event::KeyCode, crossterm::event::KeyModifiers)> {
    use crossterm::event::{KeyCode, KeyModifiers};
    let (key, modifiers) = if let Some(key) = value.strip_prefix("Ctrl+Alt+") {
        (key, KeyModifiers::CONTROL | KeyModifiers::ALT)
    } else if let Some(key) = value.strip_prefix("Alt+") {
        (key, KeyModifiers::ALT)
    } else {
        (value, KeyModifiers::NONE)
    };
    if !modifiers.is_empty() && key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() {
        return Some((
            KeyCode::Char(key.to_ascii_lowercase().chars().next()?),
            modifiers,
        ));
    }
    if let Some(number) = key.strip_prefix('F').and_then(|n| n.parse::<u8>().ok())
        && (1..=24).contains(&number)
        && key == format!("F{number}")
    {
        return Some((KeyCode::F(number), modifiers));
    }
    None
}

pub fn button_label(label: &str, hotkey: &Option<String>) -> String {
    match hotkey {
        Some(key) => format!("{label} · {key}"),
        None => label.to_owned(),
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Intent {
    #[default]
    Neutral,
    Primary,
    Danger,
    Success,
    Warning,
}

impl Intent {
    pub fn color(self, theme: &Theme) -> Color {
        match self {
            Self::Neutral => theme.surface,
            Self::Primary => theme.primary,
            Self::Danger => theme.danger,
            Self::Success => theme.success,
            Self::Warning => theme.warning,
        }
    }
    pub fn foreground(self, theme: &Theme) -> Color {
        theme.text_on(self.color(theme))
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeypadMode {
    Calculator,
    Numeric,
}

impl KeypadMode {
    pub fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Calculator => &[
                "AC", "±", "%", "÷", "7", "8", "9", "×", "4", "5", "6", "−", "1", "2", "3", "+",
                "⌫", "0", ".", "=",
            ],
            Self::Numeric => &["1", "2", "3", "4", "5", "6", "7", "8", "9", "AC", "0", "⌫"],
        }
    }
    pub fn columns(self) -> usize {
        if matches!(self, Self::Calculator) {
            4
        } else {
            3
        }
    }
    pub fn height(self) -> u16 {
        (self.keys().len() / self.columns()) as u16 * 3 + 8
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonVariant {
    #[default]
    Solid,
    Plain,
}

#[allow(clippy::too_many_arguments)]
pub fn button(
    theme: &Theme,
    frame: &mut Frame,
    area: Rect,
    label: &str,
    intent: Intent,
    focused: bool,
    disabled: bool,
    variant: ButtonVariant,
) {
    if matches!(variant, ButtonVariant::Plain) {
        let mut style = Style::default().fg(if disabled {
            theme.muted
        } else if matches!(intent, Intent::Neutral) {
            theme.foreground
        } else {
            intent.color(theme)
        });
        if focused && !disabled {
            style = style.bg(theme.focus).fg(theme.text_on(theme.focus)).bold();
        }
        frame.render_widget(
            Paragraph::new(format!("[ {label} ]"))
                .alignment(Alignment::Center)
                .style(style),
            area,
        );
        return;
    }
    let background = if disabled {
        theme.background
    } else if focused {
        theme.focus
    } else {
        intent.color(theme)
    };
    let foreground = if disabled {
        theme.muted
    } else if focused {
        theme.text_on(theme.focus)
    } else {
        intent.foreground(theme)
    };
    let border = if focused {
        theme.foreground
    } else {
        theme.border
    };
    frame.render_widget(
        Paragraph::new(label.to_owned())
            .alignment(Alignment::Center)
            .style(Style::default().fg(foreground).bg(background).bold())
            .block(
                theme
                    .block(BorderType::Rounded)
                    .border_style(Style::default().fg(border)),
            ),
        area,
    );
}

pub fn keypad(
    theme: &Theme,
    frame: &mut Frame,
    area: Rect,
    title: &str,
    value: &str,
    mode: KeypadMode,
    selected: Option<usize>,
) {
    let focused = selected.is_some();
    let width = area.width.min(if matches!(mode, KeypadMode::Calculator) {
        38
    } else {
        30
    });
    let area = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y,
        width,
        area.height.min(mode.height()),
    );
    let shell = theme
        .block(BorderType::Rounded)
        .title(format!(" {title} "))
        .style(Style::default().bg(theme.background).fg(theme.foreground))
        .border_style(Style::default().fg(if focused { theme.focus } else { theme.border }));
    let inner = shell.inner(area);
    frame.render_widget(shell, area);
    if inner.width < 4 || inner.height < 2 {
        return;
    }
    let inner = inner.inner(Margin::new(1, 0));
    let display = Rect::new(inner.x, inner.y, inner.width, inner.height.min(3));
    let visible: String = value
        .chars()
        .rev()
        .take(inner.width.saturating_sub(2) as usize)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    frame.render_widget(
        Paragraph::new(visible)
            .alignment(Alignment::Right)
            .style(
                Style::default()
                    .fg(theme.foreground)
                    .bg(theme.surface)
                    .bold(),
            )
            .block(Block::default().padding(Padding::new(1, 1, 1, 0))),
        display,
    );
    let columns = mode.columns();
    for (row, keys) in mode.keys().chunks(columns).enumerate() {
        let y = inner.y.saturating_add(4 + row as u16 * 3);
        if y >= inner.bottom() {
            break;
        }
        let row_area = Rect::new(inner.x, y, inner.width, 3.min(inner.bottom() - y));
        let cells = Layout::horizontal(vec![Constraint::Fill(1); columns])
            .spacing(1)
            .split(row_area);
        for (col, label) in keys.iter().enumerate() {
            let intent = if *label == "=" {
                Intent::Primary
            } else if ["÷", "×", "−", "+"].contains(label) {
                Intent::Warning
            } else {
                Intent::Neutral
            };
            button(
                theme,
                frame,
                cells[col],
                label,
                intent,
                selected == Some(row * columns + col),
                false,
                ButtonVariant::Solid,
            );
        }
    }
    let y = inner.y.saturating_add(mode.height() - 3);
    if y < inner.bottom() {
        frame.render_widget(
            Paragraph::new(if focused {
                "Arrows select · Enter press"
            } else {
                "Tab to use keypad"
            })
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.muted)),
            Rect::new(inner.x, y, inner.width, 1),
        );
    }
}
