use ratatui::{
    style::{Color, Style},
    widgets::{Block, BorderType},
};
use serde::{Deserialize, Serialize};

/// One palette shared by every generated primitive. Artwork keeps its authored colors.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<Design>,
    #[serde(with = "hex")]
    pub background: Color,
    #[serde(with = "hex")]
    pub surface: Color,
    #[serde(with = "hex")]
    pub foreground: Color,
    #[serde(with = "hex")]
    pub muted: Color,
    #[serde(with = "hex")]
    pub border: Color,
    #[serde(with = "hex")]
    pub primary: Color,
    #[serde(with = "hex")]
    pub focus: Color,
    #[serde(with = "hex")]
    pub danger: Color,
    #[serde(with = "hex")]
    pub success: Color,
    #[serde(with = "hex")]
    pub warning: Color,
}

/// Shared chrome; explicit panel/table properties still win. Legacy documents omit it.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Design {
    pub border: Border,
    pub density: Density,
    pub emphasis: Emphasis,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Border {
    Plain,
    Rounded,
    Double,
    Heavy,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    Compact,
    Comfortable,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Emphasis {
    Quiet,
    Bold,
}
impl Design {
    pub fn border_name(self) -> &'static str {
        match self.border {
            Border::Plain => "plain",
            Border::Rounded => "rounded",
            Border::Double => "double",
            Border::Heavy => "heavy",
        }
    }
    pub fn spacing(self) -> u16 {
        if matches!(self.density, Density::Comfortable) {
            1
        } else {
            0
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            design: None,
            background: Color::Rgb(13, 17, 23),
            surface: Color::Rgb(35, 41, 50),
            foreground: Color::Rgb(232, 228, 217),
            muted: Color::Rgb(168, 162, 150),
            border: Color::Rgb(65, 73, 84),
            primary: Color::Rgb(255, 176, 0),
            focus: Color::Rgb(255, 201, 77),
            danger: Color::Rgb(229, 72, 77),
            success: Color::Rgb(63, 185, 80),
            warning: Color::Rgb(210, 153, 34),
        }
    }
}

impl Theme {
    pub fn block(&self, fallback: BorderType) -> Block<'static> {
        let border = self.design.map_or(fallback, |d| match d.border {
            Border::Plain => BorderType::Plain,
            Border::Rounded => BorderType::Rounded,
            Border::Double => BorderType::Double,
            Border::Heavy => BorderType::Thick,
        });
        Block::bordered()
            .border_type(border)
            .title_style(self.heading())
    }
    pub fn heading(&self) -> Style {
        match self.design.map(|d| d.emphasis) {
            Some(Emphasis::Quiet) => Style::default().fg(self.foreground),
            Some(Emphasis::Bold) => Style::default().fg(self.primary).bold(),
            None => Style::default(),
        }
    }
    pub(crate) fn inherit(&self, kind: &str, props: &mut serde_json::Value) {
        let Some(props) = props.as_object_mut() else {
            return;
        };
        let keys: &[&str] = match kind {
            "Column" | "Row" | "Grid" => &["gap"],
            "Panel" => &["border", "gap", "padding"],
            "Table" => &["border", "padding", "columnGap"],
            _ => &[],
        };
        for &key in keys {
            if props.get(key).is_some_and(serde_json::Value::is_null) {
                props.remove(key);
            }
        }
        let Some(design) = self.design else {
            return;
        };
        if matches!(kind, "Panel" | "Table") {
            props
                .entry("border")
                .or_insert(serde_json::json!(design.border_name()));
            props
                .entry("padding")
                .or_insert(serde_json::json!(design.spacing()));
        }
        if matches!(kind, "Column" | "Row" | "Grid" | "Panel") {
            props
                .entry("gap")
                .or_insert(serde_json::json!(design.spacing()));
        }
        if kind == "Table" {
            props
                .entry("columnGap")
                .or_insert(serde_json::json!(design.spacing() + 1));
        }
    }

    /// Keep labels readable on both light and dark model-selected surfaces.
    pub fn text_on(&self, background: Color) -> Color {
        let bg = luminance(background);
        if contrast(self.foreground, background) >= 4.5 {
            self.foreground
        } else if bg > 0.179 {
            Color::Black
        } else {
            Color::White
        }
    }
}

fn luminance(color: Color) -> f64 {
    let Color::Rgb(r, g, b) = color else {
        return 0.0;
    };
    let linear = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

pub(crate) fn contrast(a: Color, b: Color) -> f64 {
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

pub(crate) fn parse_color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let rgb = u32::from_str_radix(hex, 16).ok()?;
    Some(Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
}

mod hex {
    use super::*;
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Color, D::Error> {
        let value = String::deserialize(deserializer)?;
        parse_color(&value).ok_or_else(|| serde::de::Error::custom("Color must be #RRGGBB"))
    }
    pub fn serialize<S: serde::Serializer>(
        color: &Color,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let Color::Rgb(r, g, b) = color else {
            return Err(serde::ser::Error::custom("Theme requires RGB colors"));
        };
        serializer.serialize_str(&format!("#{r:02x}{g:02x}{b:02x}"))
    }
}

/// Inclusive lower bounds, shared by numeric displays and charts.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ColorThreshold {
    pub min: f64,
    pub color: String,
}

pub(crate) fn numeric_value(text: &str) -> Option<f64> {
    text.trim()
        .strip_suffix('%')
        .unwrap_or(text.trim())
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

pub(crate) fn validate_value_colors(
    color: &Option<String>,
    thresholds: &[ColorThreshold],
) -> crate::Result<()> {
    let theme = Theme::default();
    if color
        .as_ref()
        .is_some_and(|c| theme.named_color(c).is_none())
        || thresholds.len() > 16
        || thresholds
            .iter()
            .any(|t| !t.min.is_finite() || theme.named_color(&t.color).is_none())
        || thresholds.windows(2).any(|pair| pair[0].min >= pair[1].min)
    {
        return Err("Colors require a theme role, green/yellow/red, or #RRGGBB; thresholds require at most 16 strictly increasing finite min values".into());
    }
    Ok(())
}

impl Theme {
    fn named_color(&self, color: &str) -> Option<Color> {
        Some(match color {
            "foreground" => self.foreground,
            "muted" => self.muted,
            "primary" => self.primary,
            "success" => self.success,
            "warning" => self.warning,
            "danger" => self.danger,
            "green" => Color::Rgb(34, 197, 94),
            "yellow" => Color::Rgb(234, 179, 8),
            "red" => Color::Rgb(239, 68, 68),
            _ => return parse_color(color),
        })
    }
    pub(crate) fn value_color(
        &self,
        color: &Option<String>,
        thresholds: &[ColorThreshold],
        value: Option<f64>,
    ) -> Option<Color> {
        thresholds
            .iter()
            .rev()
            .find(|t| value.is_some_and(|v| v >= t.min))
            .map(|t| t.color.as_str())
            .or(color.as_deref())
            .and_then(|c| self.named_color(c))
    }
}
