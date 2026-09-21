//! Measurements use the same boxes as drawing and keyboard focus.
use crate::{Spec, Widget, Width, clean};
use ratatui::prelude::*;
use serde_json::{Value, json};

fn text_width(text: &str) -> u16 {
    text.lines()
        .map(|line| Line::from(clean(line)).width())
        .max()
        .unwrap_or(0)
        .min(4096) as u16
}

impl Spec {
    pub(super) fn content_width(&self, id: &str, available: u16) -> u16 {
        let widget = self.widget(id).expect("validated widget");
        let width = match widget {
            Widget::Column { .. }
            | Widget::Row { .. }
            | Widget::Grid { .. }
            | Widget::Panel { .. } => {
                let layout = self.container_layout(id, available);
                let widths = layout
                    .rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|child| {
                                let sizing = self.sizing(child);
                                let natural = match sizing.width {
                                    Some(Width::Cells(width)) => width,
                                    _ => self.content_width(child, available),
                                };
                                if sizing.max_width > 0 {
                                    natural.min(sizing.max_width)
                                } else {
                                    natural
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let mut content = widths
                    .iter()
                    .map(|row| {
                        row.iter()
                            .copied()
                            .fold(0u16, u16::saturating_add)
                            .saturating_add(
                                layout
                                    .gap
                                    .saturating_mul(row.len().saturating_sub(1) as u16),
                            )
                    })
                    .max()
                    .unwrap_or(0);
                if matches!(widget, Widget::Grid { .. }) {
                    content = widths
                        .iter()
                        .flatten()
                        .copied()
                        .max()
                        .unwrap_or(0)
                        .saturating_mul(layout.columns as u16)
                        .saturating_add(
                            layout
                                .gap
                                .saturating_mul(layout.columns.saturating_sub(1) as u16),
                        );
                }
                let inner = self.inner_area(id, Rect::new(0, 0, 4096, 4096));
                let border = 4096 - inner.width;
                let title = if let Widget::Panel { title, .. } = widget {
                    text_width(&title)
                } else {
                    0
                };
                content.max(title).saturating_add(border)
            }
            Widget::Text { text, .. } => text_width(&text),
            Widget::Metric { label, value, .. } => text_width(&label).max(text_width(&value)),
            Widget::Input { label, value } => text_width(&label)
                .max(text_width(&value).saturating_add(1))
                .saturating_add(2),
            Widget::Button { label, hotkey, .. } => {
                text_width(&crate::controls::button_label(&label, &hotkey)).saturating_add(4)
            }
            Widget::Popup { label, .. } => text_width(&label).saturating_add(4),
            Widget::Badge { label, .. } => text_width(&label).saturating_add(2),
            Widget::Switch { label, .. } => text_width(&label).saturating_add(9),
            Widget::List { title, items } => items
                .iter()
                .map(|s| text_width(s))
                .max()
                .unwrap_or(0)
                .max(text_width(&title))
                .saturating_add(2),
            Widget::Select {
                title,
                options,
                searchable,
                ..
            }
            | Widget::MultiSelect {
                title,
                options,
                searchable,
                ..
            } => {
                let multiple = self.elements[id].kind == "MultiSelect";
                let bordered = self.elements[id].props["bordered"]
                    .as_bool()
                    .unwrap_or(true);
                let option = options.iter().map(|s| text_width(s)).max().unwrap_or(0);
                let hint = if searchable {
                    if multiple {
                        text_width("100 selected · Space toggle")
                    } else {
                        option.saturating_add(text_width("Selected:  · Enter choose"))
                    }
                } else if multiple {
                    text_width("Space toggle · Tab next")
                } else {
                    0
                };
                option
                    .saturating_add(if multiple { 6 } else { 4 })
                    .max(hint)
                    .max(if bordered { text_width(&title) } else { 0 })
                    .saturating_add(if bordered { 2 } else { 0 })
            }
            Widget::ScrollView { title, text, .. } => {
                text_width(&text).max(text_width(&title)).saturating_add(2)
            }
            Widget::BigText { text, .. } => (text.len() as u16).saturating_mul(4),
            Widget::AnsiArt { title, content, .. } => {
                use ansi_to_tui::IntoText;
                let width = crate::widgets::ansi_content(&content)
                    .into_text()
                    .unwrap_or_default()
                    .width()
                    .min(4096) as u16;
                width.max(text_width(&title)).saturating_add(2)
            }
            Widget::Scene { title, rows, .. } => rows
                .iter()
                .map(|s| text_width(s))
                .max()
                .unwrap_or(0)
                .max(text_width(&title))
                .saturating_add(2),
            Widget::PlayingCards { title, cards } => (cards.len() as u16 * 15)
                .max(text_width(&title))
                .saturating_add(2),
            Widget::QRCode { title, data } => qrcode::QrCode::new(data.as_bytes())
                .map(|q| q.width() as u16 + 8)
                .unwrap_or(0)
                .max(text_width(&title))
                .saturating_add(2),
            Widget::Table(table) => {
                let widths: u16 = (0..table.headers.len())
                    .map(|i| {
                        table
                            .column_styles
                            .get(i)
                            .filter(|c| c.width > 0)
                            .map(|c| c.width)
                            .unwrap_or_else(|| {
                                table
                                    .rows
                                    .iter()
                                    .map(|row| text_width(&row[i]))
                                    .max()
                                    .unwrap_or(0)
                                    .max(if table.show_header {
                                        text_width(&table.headers[i])
                                    } else {
                                        0
                                    })
                                    .saturating_add(table.padding * 2)
                            })
                    })
                    .fold(0, u16::saturating_add);
                widths
                    .saturating_add(table.column_gap * table.headers.len().saturating_sub(1) as u16)
                    .max(text_width(&table.title))
                    .saturating_add(if table.border == "none" { 0 } else { 2 })
            }
            // Charts and other fluid graphics have no intrinsic horizontal size.
            _ => available,
        };
        width.max(1)
    }

    /// Flow geometry in document coordinates, before scrolling. No model or terminal required.
    pub fn layout_report(&self, width: u16, height: u16) -> Value {
        fn rect(r: Rect) -> [u16; 4] {
            [r.x, r.y, r.width, r.height]
        }
        fn visit(
            spec: &Spec,
            id: &str,
            parent: Option<&str>,
            slot: Rect,
            nodes: &mut serde_json::Map<String, Value>,
        ) {
            if !spec.is_visible(id) {
                return;
            }
            let bounds = spec.sized_area(id, slot);
            let inner = spec.inner_area(id, bounds);
            nodes.insert(id.into(), json!({"parent":parent,"bounds":rect(bounds),"inner":rect(inner),"slot":rect(slot)}));
            for (child, area) in spec.child_areas(id, bounds) {
                visit(spec, child, Some(id), area, nodes);
            }
            let minimum = match spec.widget(id).expect("validated widget") {
                Widget::Button { .. } => spec.content_width(id, bounds.width),
                Widget::Input { .. } => 3,
                Widget::Column { .. }
                | Widget::Row { .. }
                | Widget::Grid { .. }
                | Widget::Panel { .. } => {
                    let layout = spec.container_layout(id, bounds.width);
                    let widths: Vec<Vec<u16>> = layout
                        .rows
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|child| {
                                    nodes[*child]["minimumControlWidth"].as_u64().unwrap_or(0)
                                        as u16
                                })
                                .collect()
                        })
                        .collect();
                    let content = if matches!(spec.widget(id), Ok(Widget::Grid { .. })) {
                        widths
                            .iter()
                            .flatten()
                            .copied()
                            .max()
                            .unwrap_or(0)
                            .saturating_mul(layout.columns as u16)
                            .saturating_add(
                                layout
                                    .gap
                                    .saturating_mul(layout.columns.saturating_sub(1) as u16),
                            )
                    } else {
                        widths
                            .iter()
                            .map(|row| {
                                row.iter()
                                    .copied()
                                    .fold(0u16, u16::saturating_add)
                                    .saturating_add(
                                        layout
                                            .gap
                                            .saturating_mul(row.len().saturating_sub(1) as u16),
                                    )
                            })
                            .max()
                            .unwrap_or(0)
                    };
                    content.saturating_add(bounds.width.saturating_sub(inner.width))
                }
                _ => 0,
            };
            nodes.get_mut(id).unwrap()["minimumControlWidth"] = json!(minimum);
        }
        let content_height = self.content_height_in(width).max(height).min(4096);
        let mut nodes = serde_json::Map::new();
        visit(
            self,
            &self.root,
            None,
            Rect::new(0, 0, width, content_height),
            &mut nodes,
        );
        let mut audits = Vec::new();
        for (role, color) in [
            ("foreground", self.theme.foreground),
            ("muted", self.theme.muted),
        ] {
            for (surface, background) in [
                ("background", self.theme.background),
                ("surface", self.theme.surface),
            ] {
                let ratio = crate::theme::contrast(color, background);
                if ratio < 4.5 {
                    audits.push(json!({"id":self.root,"code":"text-contrast","severity":"warning","message":format!("{role} on {surface}: contrast {ratio:.2}:1, recommended at least 4.5:1.")}));
                }
            }
        }
        for (id, node) in &nodes {
            let bounds = node["bounds"].as_array().expect("native bounds");
            let w = bounds[2].as_u64().unwrap() as u16;
            let h = bounds[3].as_u64().unwrap() as u16;
            let widget = self.widget(id).expect("validated widget");
            let mut finding = |code: &str, severity: &str, message: String| {
                audits.push(json!({"id":id,"code":code,"severity":severity,"message":message}));
            };
            if w == 0 || h == 0 {
                finding(
                    "empty-area",
                    "error",
                    format!("Visible widget has {w}×{h} cells; requires a nonempty area."),
                );
            } else if matches!(widget, Widget::Button { .. } | Widget::Input { .. }) {
                let required = if matches!(widget, Widget::Button { .. }) {
                    self.content_width(id, w)
                } else {
                    3
                };
                let required_height = if matches!(
                    widget,
                    Widget::Button {
                        variant: crate::controls::ButtonVariant::Plain,
                        ..
                    }
                ) {
                    1
                } else {
                    3
                };
                if w < required || h < required_height {
                    finding(
                        "control-fit",
                        "error",
                        format!(
                            "Control has {w}×{h} cells; requires at least {required}×{required_height}."
                        ),
                    );
                    audits.last_mut().unwrap()["requiredSize"] = json!([required, required_height]);
                }
            } else if matches!(
                widget,
                Widget::BigText { .. } | Widget::AnsiArt { .. } | Widget::Scene { .. }
            ) {
                let required = self.content_width(id, w);
                if w < required {
                    finding(
                        "art-clipping",
                        "warning",
                        format!("Artwork width is {required} cells; allocated width is {w}."),
                    );
                }
            }
        }
        json!({"viewport":[width,height],"contentHeight":content_height,"nodes":nodes,"audits":audits})
    }
}
