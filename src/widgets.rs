use crate::{
    Result, clean,
    controls::{self, ButtonVariant, Intent, KeypadMode},
    theme::{
        ColorThreshold, Theme, numeric_value, parse_color as scene_color, validate_value_colors,
    },
};
use ansi_to_tui::IntoText;
use ratatui::{prelude::*, widgets::*};
use serde::Deserialize;
use tui_widgets::{
    big_text::{BigText, PixelSize},
    box_text::BoxChar,
    cards::{Card, Rank, Suit},
    equalizer::{Band, Equalizer},
    qrcode::{QrCodeWidget, Scaling},
    scrollview::{ScrollView, ScrollViewState},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlayingCard {
    pub rank: String,
    pub suit: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TabPage {
    pub label: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SceneLegend {
    pub symbol: String,
    pub label: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "props", deny_unknown_fields)]
pub(crate) enum Widget {
    AnsiArt {
        title: String,
        content: String,
        height: u16,
    },
    Scene {
        title: String,
        rows: Vec<String>,
        legend: Vec<SceneLegend>,
    },
    Column {
        #[serde(default)]
        gap: u16,
    },
    Row {
        #[serde(default)]
        gap: u16,
        #[serde(default, rename = "collapseBelow")]
        collapse_below: u16,
    },
    Grid {
        columns: u16,
        #[serde(default, deserialize_with = "optional_gap")]
        gap: Option<u16>,
        #[serde(default, rename = "minCellWidth")]
        min_cell_width: u16,
    },
    Panel {
        title: String,
        #[serde(default)]
        surface: bool,
        #[serde(default = "plain_border")]
        border: String,
        #[serde(default)]
        padding: u16,
        #[serde(default)]
        gap: u16,
        #[serde(default = "left_align", rename = "titleAlign")]
        title_align: String,
    },
    Text {
        #[serde(deserialize_with = "display_value")]
        text: String,
        #[serde(default)]
        muted: bool,
    },
    Metric {
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        thresholds: Vec<ColorThreshold>,
        label: String,
        #[serde(deserialize_with = "display_value")]
        value: String,
    },
    Gauge {
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        thresholds: Vec<ColorThreshold>,
        title: String,
        value: f64,
    },
    Sparkline {
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        thresholds: Vec<ColorThreshold>,
        title: String,
        data: Vec<u64>,
        #[serde(default)]
        dense: bool,
    },
    Table(crate::table::TableProps),
    List {
        title: String,
        items: Vec<String>,
    },
    Input {
        label: String,
        value: String,
    },
    Button {
        label: String,
        #[serde(default)]
        hotkey: Option<String>,
        #[serde(default)]
        variant: ButtonVariant,
        #[serde(default)]
        intent: Intent,
        #[serde(default)]
        disabled: bool,
    },
    Badge {
        label: String,
        intent: Intent,
    },
    Switch {
        label: String,
        checked: bool,
        disabled: bool,
    },
    Keypad {
        title: String,
        value: String,
        mode: KeypadMode,
    },

    BigText {
        text: String,
        font: String,
    },
    PlayingCards {
        title: String,
        cards: Vec<PlayingCard>,
    },
    QRCode {
        title: String,
        data: String,
    },
    Equalizer {
        title: String,
        levels: Vec<u64>,
    },
    BarChart {
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        thresholds: Vec<ColorThreshold>,
        title: String,
        labels: Vec<String>,
        values: Vec<u64>,
    },
    Chart {
        title: String,
        points: Vec<(f64, f64)>,
        kind: String,
    },
    ScrollView {
        title: String,
        text: String,
        height: u16,
    },
    Popup {
        title: String,
        label: String,
        body: String,
        open: bool,
    },
    Select {
        #[serde(default = "bordered_default")]
        bordered: bool,
        title: String,
        options: Vec<String>,
        value: String,
        #[serde(default)]
        searchable: bool,
    },
    MultiSelect {
        #[serde(default = "bordered_default")]
        bordered: bool,
        title: String,
        options: Vec<String>,
        value: Vec<String>,
        #[serde(default)]
        searchable: bool,
    },
    Tabs {
        title: String,
        tabs: Vec<TabPage>,
        value: String,
    },
    Slider {
        label: String,
        value: f64,
        min: f64,
        max: f64,
        step: f64,
    },
}

fn bordered_default() -> bool {
    true
}

impl Widget {
    pub fn validate_extra(&self) -> Result<()> {
        if let Self::Button {
            hotkey: Some(key), ..
        } = self
            && controls::hotkey(key).is_none()
        {
            return Err("Button hotkey must be Alt+letter/digit or F1–F24 (optional Alt or Ctrl+Alt modifier)".into());
        }
        match self {
            Self::Metric {
                color,
                thresholds,
                value,
                ..
            } => {
                validate_value_colors(color, thresholds)?;
                if !thresholds.is_empty() && numeric_value(value).is_none() {
                    return Err(
                        "Metric thresholds require a numeric value (optionally suffixed with %)"
                            .into(),
                    );
                }
            }
            Self::Gauge {
                color, thresholds, ..
            }
            | Self::Sparkline {
                color, thresholds, ..
            }
            | Self::BarChart {
                color, thresholds, ..
            } => validate_value_colors(color, thresholds)?,
            _ => {}
        }
        let valid = match self {
            Self::Column { gap } => *gap <= 4,
            Self::Row {
                gap,
                collapse_below,
            } => *gap <= 4 && *collapse_below <= 240,
            Self::Grid {
                columns,
                gap,
                min_cell_width,
            } => (1..=8).contains(columns) && gap.is_none_or(|g| g <= 4) && *min_cell_width <= 120,
            Self::Panel {
                border,
                padding,
                gap,
                title_align,
                ..
            } => {
                ["plain", "rounded", "heavy", "double", "none"].contains(&border.as_str())
                    && *padding <= 4
                    && *gap <= 4
                    && ["left", "center", "right"].contains(&title_align.as_str())
            }
            Self::Table(table) => return table.validate(),
            Self::MultiSelect { options, value, .. } => {
                (1..=100).contains(&options.len())
                    && unique(options.iter().map(String::as_str))
                    && unique(value.iter().map(String::as_str))
                    && value.iter().all(|s| options.contains(s))
                    && options
                        .iter()
                        .all(|s| !s.is_empty() && s.chars().count() <= 80)
            }
            Self::AnsiArt {
                content, height, ..
            } => content.chars().count() <= 16000 && (4..=26).contains(height),
            Self::Scene { rows, legend, .. } => {
                (1..=24).contains(&rows.len())
                    && (1..=12).contains(&legend.len())
                    && unique(legend.iter().map(|entry| entry.symbol.as_str()))
                    && legend.iter().all(|entry| {
                        entry.symbol.len() == 1
                            && entry.symbol.bytes().all(|b| (32..=126).contains(&b))
                            && !entry.label.is_empty()
                            && entry.label.chars().count() <= 24
                            && scene_color(&entry.color).is_some()
                    })
                    && rows.iter().all(|row| {
                        (1..=64).contains(&row.len())
                            && row.bytes().all(|b| (32..=126).contains(&b))
                    })
            }
            Self::BigText { text, font } => {
                !text.is_empty()
                    && text.len() <= 24
                    && text.bytes().all(|b| (32..=126).contains(&b))
                    && ["pixel", "box"].contains(&font.as_str())
            }
            Self::PlayingCards { cards, .. } => {
                (1..=8).contains(&cards.len()) && cards.iter().all(|c| card(c).is_some())
            }
            Self::QRCode { data, .. } => {
                !data.is_empty()
                    && data.chars().count() <= 120
                    && qrcode::QrCode::new(data.as_bytes()).is_ok()
            }
            Self::Equalizer { levels, .. } => {
                (1..=32).contains(&levels.len()) && levels.iter().all(|n| *n <= 100)
            }
            Self::BarChart { labels, values, .. } => {
                (1..=12).contains(&labels.len())
                    && labels.len() == values.len()
                    && labels.iter().all(|s| s.chars().count() <= 16)
                    && values.iter().all(|n| *n <= 1_000_000_000)
            }
            Self::Chart { points, kind, .. } => {
                (2..=64).contains(&points.len())
                    && ["line", "scatter"].contains(&kind.as_str())
                    && points.iter().all(|(x, y)| {
                        x.is_finite()
                            && y.is_finite()
                            && x.abs() <= 1_000_000.0
                            && y.abs() <= 1_000_000.0
                    })
            }
            Self::ScrollView { text, height, .. } => {
                text.chars().count() <= 12000 && (4..=20).contains(height)
            }
            Self::Popup { body, .. } => body.chars().count() <= 2000,
            Self::Select { options, value, .. } => {
                (1..=100).contains(&options.len())
                    && options.contains(value)
                    && unique(options.iter().map(String::as_str))
                    && options
                        .iter()
                        .all(|s| !s.is_empty() && s.chars().count() <= 80)
            }
            Self::Tabs { tabs, value, .. } => {
                (1..=8).contains(&tabs.len())
                    && tabs.iter().any(|t| &t.label == value)
                    && unique(tabs.iter().map(|t| t.label.as_str()))
                    && tabs.iter().all(|t| {
                        !t.label.is_empty()
                            && t.label.chars().count() <= 40
                            && t.text.chars().count() <= 2000
                    })
            }
            Self::Slider {
                value,
                min,
                max,
                step,
                ..
            } => {
                [value, min, max, step].iter().all(|n| n.is_finite())
                    && *min >= 0.0
                    && *max <= 100.0
                    && min < max
                    && value >= min
                    && value <= max
                    && (0.1..=100.0).contains(step)
            }
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err("Invalid widget data, selection, or bounds".into())
        }
    }

    pub fn extra_height(&self) -> u16 {
        match self {
            Self::AnsiArt {
                height, content, ..
            } => (*height).max(
                ansi_content(content)
                    .into_text()
                    .unwrap_or_default()
                    .height() as u16
                    + 2,
            ),
            Self::Scene { rows, .. } => rows.len() as u16 + 5,
            Self::BigText { font, .. } => {
                if font == "box" {
                    4
                } else {
                    5
                }
            }
            Self::PlayingCards { .. } => 12,
            Self::QRCode { data, .. } => qrcode::QrCode::new(data.as_bytes())
                .map(|q| ((q.width() + 8).div_ceil(2) + 2) as u16)
                .unwrap_or(3),
            Self::Equalizer { .. }
            | Self::BarChart { .. }
            | Self::Chart { .. }
            | Self::Tabs { .. } => 10,
            Self::ScrollView { height, .. } => *height,
            Self::Select {
                options,
                searchable,
                bordered,
                ..
            }
            | Self::MultiSelect {
                options,
                searchable,
                bordered,
                ..
            } => {
                options.len().min(6) as u16
                    + if *bordered { 2 } else { 0 }
                    + if *searchable { 2 } else { 0 }
            }
            Self::Button {
                variant: ButtonVariant::Plain,
                ..
            } => 1,
            _ => 3,
        }
    }

    pub fn render_extra(
        self,
        theme: &Theme,
        frame: &mut Frame,
        area: Rect,
        focused: bool,
        scroll: &mut ScrollViewState,
    ) {
        let panel = |title: &str| {
            theme
                .block(BorderType::Rounded)
                .title(clean(title))
                .border_style(Style::default().fg(if focused { theme.focus } else { theme.border }))
        };
        match self {
            Self::AnsiArt { title, content, .. } => {
                let border = panel(&title);
                let inner = border.inner(area);
                frame.render_widget(border, area);
                let content = ansi_content(&content);
                let mut text = content.into_text().unwrap_or_default();
                // Only styled cells reach the terminal; never replay terminal commands.
                for line in &mut text.lines {
                    for span in &mut line.spans {
                        span.content = span
                            .content
                            .chars()
                            .filter(|c| !c.is_control())
                            .collect::<String>()
                            .into();
                    }
                }
                let width = (text.width().min(usize::from(inner.width))) as u16;
                let centered = Rect::new(
                    inner.x + (inner.width - width) / 2,
                    inner.y,
                    width,
                    inner.height,
                );
                frame.render_widget(Paragraph::new(text), centered);
            }
            Self::Scene {
                title,
                rows,
                legend,
            } => {
                let border = panel(&title);
                let inner = border.inner(area);
                frame.render_widget(border, area);
                let map_width = rows.iter().map(String::len).max().unwrap_or(1);
                let scale = if inner.width >= map_width as u16 * 2 {
                    2
                } else {
                    1
                };
                let lines: Vec<Line> = rows
                    .iter()
                    .map(|row| {
                        let row = format!("{row:map_width$}");
                        Line::from(
                            row.chars()
                                .map(|symbol| {
                                    let color = legend
                                        .iter()
                                        .find(|entry| entry.symbol.starts_with(symbol))
                                        .and_then(|entry| scene_color(&entry.color))
                                        .unwrap_or(theme.muted);
                                    Span::styled(
                                        if scale == 2 {
                                            format!("{symbol} ")
                                        } else {
                                            symbol.to_string()
                                        },
                                        Style::default().fg(color),
                                    )
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect();
                let parts = Layout::vertical([
                    Constraint::Length(rows.len() as u16 + 1),
                    Constraint::Min(0),
                ])
                .split(inner);
                frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), parts[0]);
                let legend = Line::from(
                    legend
                        .iter()
                        .map(|entry| {
                            Span::styled(
                                format!("{} {}  ", entry.symbol, clean(&entry.label)),
                                Style::default()
                                    .fg(scene_color(&entry.color).unwrap_or(theme.muted)),
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                frame.render_widget(
                    Paragraph::new(legend)
                        .wrap(Wrap { trim: true })
                        .alignment(Alignment::Center),
                    parts[1],
                );
            }
            Self::BigText { text, font } => {
                if font == "box" {
                    for (i, c) in text.chars().enumerate() {
                        let x = area.x.saturating_add(i as u16 * 4);
                        if x >= area.right() {
                            break;
                        }
                        frame.render_widget(
                            &BoxChar::new(c),
                            Rect::new(x, area.y, 4.min(area.right() - x), 3.min(area.height)),
                        );
                    }
                } else {
                    frame.render_widget(
                        BigText::builder()
                            .pixel_size(PixelSize::Quadrant)
                            .lines(vec![
                                Line::from(text).style(Style::default().fg(theme.primary)),
                            ])
                            .build(),
                        area,
                    );
                }
            }
            Self::PlayingCards { title, cards } => {
                let border = panel(&title);
                let inner = border.inner(area);
                frame.render_widget(border, area);
                let columns =
                    Layout::horizontal(vec![Constraint::Fill(1); cards.len()]).split(inner);
                for (value, rect) in cards.iter().zip(columns.iter()) {
                    if rect.width >= 15 && rect.height >= 10 {
                        if let Some(value) = card(value) {
                            frame.render_widget(&value, Rect::new(rect.x, rect.y, 15, 10));
                        }
                    } else {
                        let suit = match value.suit.as_str() {
                            "hearts" => "♥",
                            "diamonds" => "♦",
                            "clubs" => "♣",
                            _ => "♠",
                        };
                        let color = if ["hearts", "diamonds"].contains(&value.suit.as_str()) {
                            theme.danger
                        } else {
                            theme.primary
                        };
                        frame.render_widget(
                            Paragraph::new(format!("{}{suit}", value.rank))
                                .style(Style::default().fg(color).bold())
                                .block(theme.block(BorderType::Plain)),
                            *rect,
                        );
                    }
                }
            }
            Self::QRCode { title, data } => {
                let border = panel(&title);
                let inner = border.inner(area);
                frame.render_widget(border, area);
                if let Ok(qr) = qrcode::QrCode::new(data.as_bytes()) {
                    let widget = QrCodeWidget::new(qr)
                        .scaling(Scaling::Exact(1, 1))
                        .style(Style::default().fg(Color::Black).bg(Color::White));
                    let size = widget.size(inner);
                    if size.width <= inner.width && size.height <= inner.height {
                        frame.render_widget(
                            widget,
                            Rect::new(
                                inner.x + (inner.width - size.width) / 2,
                                inner.y,
                                size.width,
                                size.height,
                            ),
                        );
                    } else {
                        frame.render_widget(Paragraph::new("Widen this pane to scan QR"), inner);
                    }
                }
            }
            Self::Equalizer { title, levels } => {
                let border = panel(&title);
                let inner = border.inner(area);
                frame.render_widget(border, area);
                if inner.is_empty() {
                    return;
                }
                frame.render_widget(
                    Equalizer {
                        bands: levels
                            .into_iter()
                            .take(usize::from(inner.width.div_ceil(2)))
                            .map(|n| Band::from(n as f64 / 100.0))
                            .collect(),
                        brightness: 1.0,
                    },
                    inner,
                );
            }
            Self::BarChart {
                color,
                thresholds,
                title,
                labels,
                values,
            } => {
                let labels: Vec<_> = labels.iter().map(|s| clean(s)).collect();
                let data: Vec<_> = labels
                    .iter()
                    .zip(values.iter())
                    .map(|(s, n)| {
                        let color = theme
                            .value_color(&color, &thresholds, Some(*n as f64))
                            .unwrap_or(theme.primary);
                        Bar::default()
                            .label(Line::from(s.as_str()))
                            .value(*n)
                            .style(Style::default().fg(color))
                            .value_style(Style::default().fg(theme.text_on(color)).bg(color))
                    })
                    .collect();
                frame.render_widget(
                    BarChart::default()
                        .block(panel(&title))
                        .bar_width(
                            (area.width.saturating_sub(2) / data.len().max(1) as u16)
                                .saturating_sub(1)
                                .max(1),
                        )
                        .bar_gap(1)
                        .max(values.iter().copied().max().unwrap_or(1).max(1))
                        .data(BarGroup::default().bars(&data))
                        .bar_style(Style::default().fg(theme.primary))
                        .value_style(
                            Style::default()
                                .fg(theme.text_on(theme.primary))
                                .bg(theme.primary),
                        ),
                    area,
                );
            }
            Self::Chart {
                title,
                points,
                kind,
            } => {
                let (mut x0, mut x1, mut y0, mut y1) = (
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                );
                for (x, y) in &points {
                    x0 = x0.min(*x);
                    x1 = x1.max(*x);
                    y0 = y0.min(*y);
                    y1 = y1.max(*y);
                }
                if x0 == x1 {
                    x0 -= 1.0;
                    x1 += 1.0;
                }
                if y0 == y1 {
                    y0 -= 1.0;
                    y1 += 1.0;
                }
                let dataset = Dataset::default()
                    .data(&points)
                    .graph_type(if kind == "line" {
                        GraphType::Line
                    } else {
                        GraphType::Scatter
                    })
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(theme.primary));
                frame.render_widget(
                    Chart::new(vec![dataset])
                        .block(panel(&title))
                        .x_axis(
                            Axis::default()
                                .bounds([x0, x1])
                                .labels([format!("{x0:.1}"), format!("{x1:.1}")]),
                        )
                        .y_axis(
                            Axis::default()
                                .bounds([y0, y1])
                                .labels([format!("{y0:.1}"), format!("{y1:.1}")]),
                        ),
                    area,
                );
            }
            Self::ScrollView { title, text, .. } => {
                let border = panel(&title);
                let inner = border.inner(area);
                frame.render_widget(border, area);
                scroll_text(frame, inner, &text, scroll);
            }
            Self::Popup { label, .. } => controls::button(
                theme,
                frame,
                area,
                &clean(&label),
                Intent::Neutral,
                focused,
                false,
                ButtonVariant::Solid,
            ),
            Self::Select {
                title,
                options,
                value,
                bordered,
                ..
            } => {
                let selected = options.iter().position(|s| s == &value).unwrap_or(0);
                let mut state = ListState::default().with_selected(Some(selected));
                frame.render_stateful_widget(
                    List::new(options.iter().map(|s| clean(s)))
                        .block(if bordered {
                            panel(&title)
                        } else {
                            Block::default()
                        })
                        .highlight_symbol("› ")
                        .highlight_style(if focused {
                            Style::default()
                                .fg(theme.text_on(theme.primary))
                                .bg(theme.primary)
                                .bold()
                        } else {
                            Style::default().fg(theme.foreground).bold()
                        }),
                    area,
                    &mut state,
                );
            }
            Self::Tabs { title, tabs, value } => {
                let border = panel(&title);
                let inner = border.inner(area);
                frame.render_widget(border, area);
                let parts =
                    Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).split(inner);
                let selected = tabs.iter().position(|t| t.label == value).unwrap_or(0);
                frame.render_widget(
                    Tabs::new(tabs.iter().map(|t| clean(&t.label)))
                        .select(selected)
                        .highlight_style(Style::default().fg(theme.primary).bold())
                        .divider("│"),
                    parts[0],
                );
                if let Some(tab) = tabs.get(selected) {
                    scroll_text(frame, parts[1], &tab.text, scroll);
                }
            }
            Self::Slider {
                label,
                value,
                min,
                max,
                ..
            } => frame.render_widget(
                LineGauge::default()
                    .block(panel(&label))
                    .ratio((value - min) / (max - min))
                    .label(format!("{value:.1}"))
                    .filled_style(Style::default().fg(theme.primary))
                    .unfilled_style(Style::default().fg(theme.border)),
                area,
            ),
            _ => unreachable!("base widgets render in Spec"),
        }
    }
}

fn unique<'a>(items: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = std::collections::HashSet::new();
    items.into_iter().all(|s| seen.insert(s))
}

fn display_value<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(value) => Ok(value),
        value @ (serde_json::Value::Number(_) | serde_json::Value::Bool(_)) => {
            Ok(value.to_string())
        }
        _ => Err(serde::de::Error::custom(
            "Display value must be text, a number, or a boolean",
        )),
    }
}

fn card(value: &PlayingCard) -> Option<Card> {
    let rank = match value.rank.as_str() {
        "A" => Rank::Ace,
        "2" => Rank::Two,
        "3" => Rank::Three,
        "4" => Rank::Four,
        "5" => Rank::Five,
        "6" => Rank::Six,
        "7" => Rank::Seven,
        "8" => Rank::Eight,
        "9" => Rank::Nine,
        "10" => Rank::Ten,
        "J" => Rank::Jack,
        "Q" => Rank::Queen,
        "K" => Rank::King,
        _ => return None,
    };
    let suit = match value.suit.as_str() {
        "clubs" => Suit::Clubs,
        "diamonds" => Suit::Diamonds,
        "hearts" => Suit::Hearts,
        "spades" => Suit::Spades,
        _ => return None,
    };
    Some(Card::new(rank, suit))
}

fn scroll_text(frame: &mut Frame, area: Rect, text: &str, state: &mut ScrollViewState) {
    if area.is_empty() {
        return;
    }
    let text: String = text
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(12000)
        .collect();
    let width = text
        .lines()
        .map(|l| Line::from(l).width())
        .max()
        .unwrap_or(1)
        .clamp(1, 512) as u16;
    let height = text.lines().count().clamp(1, 1000) as u16;
    let mut view = ScrollView::new(Size::new(width, height));
    view.render_widget(Paragraph::new(text), Rect::new(0, 0, width, height));
    frame.render_stateful_widget(view, area, state);
}

pub(crate) fn plain_border() -> String {
    "plain".into()
}
pub(crate) fn left_align() -> String {
    "left".into()
}
pub(crate) fn panel_block(title: &str, border: &str, padding: u16, align: &str) -> Block<'static> {
    let block = Block::default().padding(Padding::uniform(padding));
    let block = if border == "none" {
        block
    } else {
        block.borders(Borders::ALL).border_type(match border {
            "rounded" => BorderType::Rounded,
            "heavy" => BorderType::Thick,
            "double" => BorderType::Double,
            _ => BorderType::Plain,
        })
    };
    if title.is_empty() {
        block
    } else {
        block.title(clean(title)).title_alignment(match align {
            "center" => Alignment::Center,
            "right" => Alignment::Right,
            _ => Alignment::Left,
        })
    }
}

fn optional_gap<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<u16>, D::Error> {
    u16::deserialize(deserializer).map(Some)
}

// Use the same decoded content for measurement and painting, including pasted escapes.
pub(crate) fn ansi_content(content: &str) -> String {
    if content.contains("\\u001b") || content.contains("\\x1b") || content.contains("\\033") {
        content
            .replace("\\u001b", "\u{1b}")
            .replace("\\x1b", "\u{1b}")
            .replace("\\033", "\u{1b}")
            .replace("\\n", "\n")
    } else {
        content.to_owned()
    }
}
