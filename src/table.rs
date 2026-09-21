//! Native table primitive with strict, bounded props.
use crate::{clean, theme::Theme};
use ratatui::{prelude::*, widgets::*};
use serde::Deserialize;

/// Per-column layout + styling.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ColumnStyle {
    /// 0 = share remaining width (Fill 1), else fixed cell count.
    #[serde(default)]
    pub width: u16,
    #[serde(default = "default_align")]
    pub align: String,
    #[serde(default = "default_color")]
    pub color: String,
}

fn default_align() -> String {
    "left".into()
}
fn default_color() -> String {
    "foreground".into()
}

/// Table content and layout.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TableProps {
    pub title: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    #[serde(default)]
    pub column_styles: Vec<ColumnStyle>,
    #[serde(default = "default_gap")]
    pub column_gap: u16,
    #[serde(default)]
    pub padding: u16,
    #[serde(default = "default_true")]
    pub show_header: bool,
    #[serde(default = "default_border")]
    pub border: String,
}

fn default_gap() -> u16 {
    1
}
fn default_true() -> bool {
    true
}
fn default_border() -> String {
    "plain".into()
}

impl TableProps {
    pub(crate) fn validate(&self) -> crate::Result<()> {
        if self.headers.is_empty() || self.headers.len() > 12 {
            return Err("Table headers must be 1..=12".into());
        }
        if self.rows.len() > 200 {
            return Err("Table rows must be <=200".into());
        }
        for row in &self.rows {
            if row.len() != self.headers.len() {
                return Err("Table row width must match headers".into());
            }
        }
        if !self.column_styles.is_empty() && self.column_styles.len() != self.headers.len() {
            return Err("Table column_styles must be empty or match headers".into());
        }
        for style in &self.column_styles {
            if style.width > 240 {
                return Err("Table column width must be <=240".into());
            }
            if !matches!(style.align.as_str(), "left" | "center" | "right") {
                return Err("Table align must be left/center/right".into());
            }
            if !matches!(
                style.color.as_str(),
                "foreground" | "muted" | "primary" | "success" | "warning" | "danger"
            ) {
                return Err("Table color role is invalid".into());
            }
        }
        if self.column_gap > 4 || self.padding > 4 {
            return Err("Table gap/padding must be <=4".into());
        }
        if !matches!(
            self.border.as_str(),
            "plain" | "rounded" | "heavy" | "double" | "none"
        ) {
            return Err("Table border is invalid".into());
        }
        Ok(())
    }

    /// Legacy plain table: rows + header + top/bottom border. Borderless title keeps top only.
    pub(crate) fn height(&self) -> u16 {
        let mut total = self.rows.len() as u16;
        if self.show_header {
            total += 1;
        }
        if self.border == "none" {
            if !clean(&self.title).is_empty() {
                total += 1;
            }
        } else {
            total += 2;
        }
        total
    }

    fn role_color(&self, theme: &Theme, role: &str) -> Color {
        match role {
            "muted" => theme.muted,
            "primary" => theme.primary,
            "success" => theme.success,
            "warning" => theme.warning,
            "danger" => theme.danger,
            _ => theme.foreground,
        }
    }

    pub(crate) fn render(&self, theme: &Theme, frame: &mut Frame, area: Rect) {
        let pad = " ".repeat(self.padding as usize);
        let column = |index: usize| -> ColumnStyle {
            self.column_styles
                .get(index)
                .cloned()
                .unwrap_or(ColumnStyle {
                    width: 0,
                    align: default_align(),
                    color: default_color(),
                })
        };
        let widths: Vec<Constraint> = (0..self.headers.len())
            .map(|i| {
                let w = column(i).width;
                if w == 0 {
                    Constraint::Fill(1)
                } else {
                    Constraint::Length(w)
                }
            })
            .collect();
        let header = Row::new(self.headers.iter().enumerate().map(|(i, h)| {
            let c = column(i);
            Line::from(format!("{pad}{}{pad}", clean(h)))
                .style(Style::default().fg(theme.primary).bold())
                .alignment(match c.align.as_str() {
                    "center" => Alignment::Center,
                    "right" => Alignment::Right,
                    _ => Alignment::Left,
                })
        }));
        let body = self.rows.iter().map(|row| {
            Row::new(row.iter().enumerate().map(|(i, cell)| {
                let c = column(i);
                Line::from(format!("{pad}{}{pad}", clean(cell)))
                    .style(Style::default().fg(self.role_color(theme, &c.color)))
                    .alignment(match c.align.as_str() {
                        "center" => Alignment::Center,
                        "right" => Alignment::Right,
                        _ => Alignment::Left,
                    })
            }))
        });
        let mut table = Table::new(body, widths)
            .column_spacing(self.column_gap)
            .style(Style::default().fg(theme.foreground));
        if self.show_header {
            table = table.header(header);
        }
        let block = crate::widgets::panel_block(&clean(&self.title), &self.border, 0, "left")
            .title_style(theme.heading())
            .border_style(Style::default().fg(theme.border));
        frame.render_widget(table.block(block), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_validates_and_measures() {
        let props: TableProps = serde_json::from_str(
            r#"{"title":"N","headers":["k","v"],"rows":[["a","1"]],
            "columnStyles":[{"width":0,"align":"right","color":"primary"},{"width":6,"align":"center","color":"muted"}],
            "columnGap":1,"padding":1,"showHeader":true,"border":"plain"}"#,
        )
        .unwrap();
        assert!(props.validate().is_ok());
        assert_eq!(props.height(), 4);
        assert_eq!(props.column_styles[0].align, "right");
        assert_eq!(props.column_styles[1].color, "muted");
        let theme = Theme::default();
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(30, 4)).unwrap();
        terminal
            .draw(|f| props.render(&theme, f, f.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(20, 2)].symbol(), "a");
        assert_eq!(buffer[(20, 2)].fg, theme.primary);
        assert_eq!(buffer[(25, 2)].symbol(), "1");
        assert_eq!(buffer[(25, 2)].fg, theme.muted);
        let bad: TableProps = serde_json::from_str(
            r#"{"title":"","headers":["a","b"],"rows":[],"columnStyles":[{"width":0}],"border":"plain"}"#,
        )
        .unwrap();
        assert!(bad.validate().is_err());
    }
}
