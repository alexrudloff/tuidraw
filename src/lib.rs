use ratatui::{prelude::*, widgets::*};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
mod actions;
mod calculator;
mod controls;
mod layout;
pub mod theme;
use controls::KeypadMode;
mod selection;
mod table;
mod widgets;
use std::cell::RefCell;
use tui_widgets::scrollview::ScrollViewState;
use widgets::Widget;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub root: String,
    pub elements: BTreeMap<String, Element>,
    #[serde(default = "empty_object")]
    pub state: Value,
    #[serde(default)]
    pub theme: theme::Theme,
    #[serde(skip)]
    key_selection: BTreeMap<String, usize>,
    #[serde(skip)]
    key_evaluated: HashSet<String>,
    #[serde(skip)]
    scroll_states: RefCell<BTreeMap<String, ScrollViewState>>,
    #[serde(skip)]
    selections: BTreeMap<String, selection::Selection>,
}

fn empty_object() -> Value {
    json!({})
}

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    #[serde(rename = "type")]
    pub kind: String,
    pub props: Value,
    #[serde(default)]
    pub children: Vec<String>,
    #[serde(default)]
    pub on: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Sizing {
    #[serde(default)]
    width: Option<Width>,
    #[serde(default)]
    grow: Option<u16>,
    #[serde(default)]
    max_width: u16,
    #[serde(default = "full_width")]
    width_percent: u16,
    #[serde(default = "widgets::left_align")]
    horizontal_align: String,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(untagged)]
enum Width {
    Cells(u16),
    Mode(WidthMode),
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum WidthMode {
    Content,
    Fill,
}
fn full_width() -> u16 {
    100
}
impl Sizing {
    fn take(props: &mut Value) -> Result<Self> {
        let props = props
            .as_object_mut()
            .ok_or("Widget props must be an object")?;
        if props.get("grow").is_some_and(Value::is_null) {
            return Err("grow must be an integer".into());
        }
        let common = [
            "width",
            "grow",
            "maxWidth",
            "widthPercent",
            "horizontalAlign",
        ]
        .into_iter()
        .filter_map(|key| props.remove(key).map(|value| (key.into(), value)))
        .collect();
        let sizing: Self =
            serde_json::from_value(Value::Object(common)).map_err(|e| e.to_string())?;
        if matches!(sizing.width, Some(Width::Cells(n)) if !(1..=240).contains(&n))
            || sizing.grow.is_some_and(|g| g > 16)
            || sizing.max_width > 240
            || !(1..=100).contains(&sizing.width_percent)
            || !["left", "center", "right"].contains(&sizing.horizontal_align.as_str())
        {
            return Err("Invalid width, grow, maxWidth, widthPercent or horizontalAlign".into());
        }
        Ok(sizing)
    }
    fn area(&self, area: Rect, content_width: u16) -> Rect {
        let mut width = match self.width {
            Some(Width::Cells(width)) => width.min(area.width),
            Some(Width::Mode(WidthMode::Content)) => content_width.min(area.width),
            Some(Width::Mode(WidthMode::Fill)) => area.width,
            None => (u32::from(area.width) * u32::from(self.width_percent) / 100) as u16,
        };
        if area.width > 0 {
            width = width.max(1);
        }
        if self.max_width > 0 {
            width = width.min(self.max_width);
        }
        let spare = area.width - width;
        let offset = match self.horizontal_align.as_str() {
            "center" => spare / 2,
            "right" => spare,
            _ => 0,
        };
        Rect::new(area.x + offset, area.y, width, area.height)
    }
}
struct NodeLayout<'a> {
    rows: Vec<Vec<&'a str>>,
    columns: usize,
    gap: u16,
    row_gap: u16,
}

fn resolve(value: &Value, state: &Value) -> Result<Value> {
    match value {
        Value::Object(map) => {
            for key in ["$state", "$bindState"] {
                if let Some(path) = map.get(key) {
                    if map.len() != 1 {
                        return Err("State references must contain only their path".into());
                    }
                    let path = path.as_str().ok_or("State path must be a string")?;
                    return state
                        .pointer(path)
                        .cloned()
                        .ok_or_else(|| format!("Missing state path: {path}"));
                }
            }
            if map.keys().any(|key| key.starts_with('$')) {
                return Err("Unsupported expression".into());
            }
            Ok(Value::Object(
                map.iter()
                    .map(|(k, v)| Ok((k.clone(), resolve(v, state)?)))
                    .collect::<Result<_>>()?,
            ))
        }
        Value::Array(values) => Ok(Value::Array(
            values
                .iter()
                .map(|v| resolve(v, state))
                .collect::<Result<_>>()?,
        )),
        _ => Ok(value.clone()),
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().is_some_and(|v| v != 0.0),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}

// This renderer deliberately supports the state-only subset used by its catalog.
fn visible(condition: &Value, state: &Value) -> Result<bool> {
    match condition {
        Value::Bool(v) => Ok(*v),
        Value::Array(parts) => parts
            .iter()
            .try_fold(true, |acc, part| Ok(visible(part, state)? && acc)),
        Value::Object(map)
            if map.len() == 1 && (map.contains_key("$and") || map.contains_key("$or")) =>
        {
            let is_and = map.contains_key("$and");
            let parts = map
                .values()
                .next()
                .unwrap()
                .as_array()
                .ok_or("Visibility operands must be arrays")?;
            parts.iter().try_fold(is_and, |acc, part| {
                let v = visible(part, state)?;
                Ok(if is_and { acc && v } else { acc || v })
            })
        }
        Value::Object(map) => {
            if map.keys().any(|k| {
                !["$state", "eq", "neq", "gt", "gte", "lt", "lte", "not"].contains(&k.as_str())
            }) {
                return Err("Unsupported visibility expression".into());
            }
            let path = map
                .get("$state")
                .and_then(Value::as_str)
                .ok_or("Visibility requires $state")?;
            let value = state
                .pointer(path)
                .ok_or("Visibility state path is missing")?;
            let mut result = true;
            let mut compared = false;
            for op in ["eq", "neq", "gt", "gte", "lt", "lte"] {
                if let Some(other) = map.get(op) {
                    let other = resolve(other, state)?;
                    compared = true;
                    result &= match op {
                        "eq" => *value == other,
                        "neq" => *value != other,
                        _ => {
                            let (a, b) = (
                                value
                                    .as_f64()
                                    .ok_or("Numeric visibility requires numbers")?,
                                other
                                    .as_f64()
                                    .ok_or("Numeric visibility requires numbers")?,
                            );
                            match op {
                                "gt" => a > b,
                                "gte" => a >= b,
                                "lt" => a < b,
                                _ => a <= b,
                            }
                        }
                    };
                }
            }
            if !compared {
                result = truthy(value);
            }
            if let Some(not) = map.get("not") {
                if not != &json!(true) {
                    return Err("Visibility not must be true".into());
                }
                result = !result;
            }
            Ok(result)
        }
        _ => Err("Invalid visibility condition".into()),
    }
}

impl Spec {
    pub fn from_parts(
        root: String,
        elements: BTreeMap<String, Element>,
        state: Value,
        theme: theme::Theme,
    ) -> Result<Self> {
        let spec = Self {
            root,
            elements,
            state,
            theme,
            key_selection: BTreeMap::new(),
            key_evaluated: HashSet::new(),
            scroll_states: RefCell::new(BTreeMap::new()),
            selections: BTreeMap::new(),
        };
        spec.validate()?;
        Ok(spec)
    }

    pub fn parse(source: &str) -> Result<Self> {
        if source.len() > 1_000_000 {
            return Err("Spec exceeds 1 MB".into());
        }
        let spec: Self = serde_json::from_str(source).map_err(|e| e.to_string())?;
        spec.validate()?;
        Ok(spec)
    }

    fn widget(&self, id: &str) -> Result<Widget> {
        let e = self
            .elements
            .get(id)
            .ok_or_else(|| format!("Missing element: {id}"))?;
        let mut props = resolve(&e.props, &self.state)?;
        self.theme.inherit(&e.kind, &mut props);
        Sizing::take(&mut props).map_err(|error| format!("{id}: {error}"))?;
        serde_json::from_value(json!({"type": e.kind, "props": props}))
            .map_err(|e| format!("{id}: {e}"))
    }

    pub fn validate(&self) -> Result<()> {
        if self.elements.is_empty() || self.elements.len() > 128 || !self.state.is_object() {
            return Err("Expected an object state and 1–128 elements".into());
        }
        fn walk(spec: &Spec, id: &str, depth: usize, seen: &mut HashSet<String>) -> Result<()> {
            if depth > 8 || !seen.insert(id.into()) {
                return Err("Tree contains a cycle, shared child, or exceeds depth 8".into());
            }
            let widget = spec.widget(id)?;
            widget.validate_extra()?;
            let e = &spec.elements[id];
            if !matches!(
                widget,
                Widget::Column { .. }
                    | Widget::Row { .. }
                    | Widget::Grid { .. }
                    | Widget::Panel { .. }
            ) && !e.children.is_empty()
            {
                return Err(format!("Leaf {id} cannot have children"));
            }
            match &widget {
                Widget::Gauge { value, .. } if !(0.0..=100.0).contains(value) => {
                    return Err("Gauge must be between 0 and 100".into());
                }
                Widget::List { items, .. } if items.len() > 200 => {
                    return Err("List exceeds 200 items".into());
                }
                Widget::Sparkline { data, .. }
                    if data.len() > 128 || data.iter().any(|v| *v > 1_000_000_000) =>
                {
                    return Err("Sparkline is out of bounds".into());
                }
                Widget::Input { .. }
                | Widget::Keypad { .. }
                | Widget::Switch { .. }
                | Widget::Popup { .. }
                | Widget::Select { .. }
                | Widget::MultiSelect { .. }
                | Widget::Tabs { .. }
                | Widget::Slider { .. } => {
                    let toggle = matches!(widget, Widget::Switch { .. } | Widget::Popup { .. });
                    let numeric = matches!(widget, Widget::Slider { .. });
                    let field = if matches!(widget, Widget::Switch { .. }) {
                        "checked"
                    } else if matches!(widget, Widget::Popup { .. }) {
                        "open"
                    } else {
                        "value"
                    };
                    let path = e.props[field]["$bindState"]
                        .as_str()
                        .ok_or("Interactive value requires $bindState")?;
                    if !path.starts_with('/')
                        || !spec.state.pointer(path).is_some_and(|v| {
                            if toggle {
                                v.is_boolean()
                            } else if numeric {
                                v.is_number()
                            } else if matches!(widget, Widget::MultiSelect { .. }) {
                                v.is_array()
                            } else {
                                v.is_string()
                            }
                        })
                    {
                        return Err(
                            "Control must bind an existing state value of the correct type".into(),
                        );
                    }
                    if let Widget::Keypad { value, .. } = &widget
                        && value.len() > 128
                    {
                        return Err("Keypad value exceeds 128 bytes".into());
                    }
                }
                _ => {}
            }
            if let Some(v) = &e.visible {
                visible(v, &spec.state)?;
            }
            for (event, binding) in &e.on {
                if !matches!(widget, Widget::Button { .. }) || event != "press" {
                    return Err("Only Button.press events are supported".into());
                }
                actions::validate(binding, &spec.state)?;
            }
            for child in &e.children {
                walk(spec, child, depth + 1, seen)?;
            }
            Ok(())
        }
        let mut seen = HashSet::new();
        walk(self, &self.root, 1, &mut seen)?;
        if self
            .elements
            .keys()
            .filter(|id| matches!(self.widget(id), Ok(Widget::Popup { open: true, .. })))
            .count()
            > 1
        {
            return Err("Only one popup can be open".into());
        }
        if seen.len() != self.elements.len() {
            return Err("Spec contains unreachable elements".into());
        }
        let mut hotkeys = HashSet::new();
        for id in self.elements.keys() {
            if let Widget::Button {
                hotkey: Some(key), ..
            } = self.widget(id)?
                && !hotkeys.insert(controls::hotkey(&key).expect("validated hotkey"))
            {
                return Err(format!("Button {id}: duplicate hotkey {key}"));
            }
        }
        if self.content_height_in(1) > 4096 {
            return Err("Rendered content exceeds 4096 rows".into());
        }
        Ok(())
    }

    fn update(&mut self, path: &str, value: Value) -> Result<()> {
        let target = self.state.pointer_mut(path).ok_or("Missing state path")?;
        let old = std::mem::replace(target, value);
        if let Err(error) = self.validate() {
            *self.state.pointer_mut(path).unwrap() = old;
            return Err(error);
        }
        Ok(())
    }

    pub fn activate(&mut self, id: &str) -> Result<()> {
        match self.widget(id)? {
            Widget::Button { disabled: true, .. } | Widget::Switch { disabled: true, .. } => {
                return Err("Control is disabled".into());
            }
            Widget::Switch { checked, .. } => {
                let path = self.elements[id].props["checked"]["$bindState"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                return self.update(&path, json!(!checked));
            }
            Widget::Popup { open, .. } => {
                let path = self.elements[id].props["open"]["$bindState"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                return self.update(&path, json!(!open));
            }
            Widget::Select { .. }
            | Widget::Tabs { .. }
            | Widget::Slider { .. }
            | Widget::ScrollView { .. } => return Ok(()),
            Widget::Keypad { mode, .. } => {
                let selected = self.key_selection.get(id).copied().unwrap_or(0) % mode.keys().len();
                return self.keypad_press(id, mode.keys()[selected]);
            }
            _ => {}
        }
        let binding = self
            .elements
            .get(id)
            .and_then(|e| e.on.get("press"))
            .ok_or("This widget has no action")?;
        let next = actions::apply(binding, &self.state)?;
        let previous = std::mem::replace(&mut self.state, next);
        if let Err(error) = self.validate() {
            self.state = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn keypad_move(&mut self, id: &str, dx: i32, dy: i32) -> Result<()> {
        let Widget::Keypad { mode, .. } = self.widget(id)? else {
            return Err("Not a keypad".into());
        };
        let selected = self.key_selection.entry(id.into()).or_default();
        let columns = mode.columns() as i32;
        let rows = mode.keys().len() as i32 / columns;
        let x = (*selected as i32 % columns + dx).rem_euclid(columns);
        let y = (*selected as i32 / columns + dy).rem_euclid(rows);
        *selected = (y * columns + x) as usize;
        Ok(())
    }

    pub fn keypad_press(&mut self, id: &str, key: &str) -> Result<()> {
        let Widget::Keypad { value, mode, .. } = self.widget(id)? else {
            return Err("Not a keypad".into());
        };
        let key = if key == "−" { "-" } else { key };
        let (value, evaluated) = match mode {
            KeypadMode::Calculator => {
                calculator::press(&value, key, self.key_evaluated.contains(id))
            }
            KeypadMode::Numeric => {
                let mut next = value;
                match key {
                    "AC" | "C" => next = "0".into(),
                    "⌫" | "Backspace" => {
                        next.pop();
                        if next.is_empty() {
                            next = "0".into();
                        }
                    }
                    key if key.len() == 1 && key.as_bytes()[0].is_ascii_digit() => {
                        if next == "0" {
                            next.clear();
                        }
                        if next.len() < 128 {
                            next.push_str(key);
                        }
                    }
                    _ => {}
                }
                (next, false)
            }
        };
        let path = self.elements[id].props["value"]["$bindState"]
            .as_str()
            .unwrap()
            .to_owned();
        self.update(&path, json!(value))?;
        if evaluated {
            self.key_evaluated.insert(id.into());
        } else {
            self.key_evaluated.remove(id);
        }
        Ok(())
    }

    pub fn keypad_type(&mut self, id: &str, key: &str) -> Result<()> {
        self.keypad_press(id, key)?;
        if let Widget::Keypad {
            mode: KeypadMode::Calculator,
            ..
        } = self.widget(id)?
        {
            // Typed arithmetic makes Enter evaluate; arrow navigation still selects any key.
            self.key_selection
                .insert(id.into(), KeypadMode::Calculator.keys().len() - 1);
        }
        Ok(())
    }

    pub fn edit_input(&mut self, id: &str, character: Option<char>) -> Result<()> {
        let path = self
            .elements
            .get(id)
            .and_then(|e| e.props["value"]["$bindState"].as_str())
            .ok_or("This widget is not an input")?
            .to_owned();
        let mut value = self
            .state
            .pointer(&path)
            .and_then(Value::as_str)
            .ok_or("Input state is not a string")?
            .to_owned();
        if let Some(c) = character {
            if !c.is_control() && value.len() < 4096 {
                value.push(c);
            }
        } else {
            value.pop();
        }
        self.update(&path, json!(value))
    }

    fn is_visible(&self, id: &str) -> bool {
        self.elements[id]
            .visible
            .as_ref()
            .is_none_or(|v| visible(v, &self.state).unwrap_or(false))
    }

    pub fn focusable(&self) -> Vec<String> {
        if let Some(id) = self.active_popup() {
            return vec![id];
        }
        fn walk(spec: &Spec, id: &str, out: &mut Vec<String>) {
            if !spec.is_visible(id) {
                return;
            }
            if matches!(
                spec.widget(id),
                Ok(Widget::Select { .. }
                    | Widget::MultiSelect { .. }
                    | Widget::Tabs { .. }
                    | Widget::Slider { .. }
                    | Widget::ScrollView { .. }
                    | Widget::Popup { .. }
                    | Widget::Input { .. }
                    | Widget::Keypad { .. }
                    | Widget::Button {
                        disabled: false,
                        ..
                    }
                    | Widget::Switch {
                        disabled: false,
                        ..
                    })
            ) {
                out.push(id.into());
            }
            for child in &spec.elements[id].children {
                walk(spec, child, out);
            }
        }
        let mut out = Vec::new();
        walk(self, &self.root, &mut out);
        out
    }

    /// Resolve only enabled, visible buttons in the active modal scope.
    pub fn hotkey_target(&self, key: crossterm::event::KeyEvent) -> Option<String> {
        use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
        if key.kind != KeyEventKind::Press {
            return None;
        }
        let code = match key.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            code => code,
        };
        let modifiers = if matches!(code, KeyCode::Char(_)) {
            key.modifiers - KeyModifiers::SHIFT
        } else {
            key.modifiers
        };
        self.focusable().into_iter().find(|id| matches!(self.widget(id),
            Ok(Widget::Button { hotkey: Some(value), .. }) if controls::hotkey(&value) == Some((code, modifiers))))
    }

    pub fn active_popup(&self) -> Option<String> {
        fn find(spec: &Spec, id: &str) -> Option<String> {
            if !spec.is_visible(id) {
                return None;
            }
            if matches!(spec.widget(id), Ok(Widget::Popup { open: true, .. })) {
                return Some(id.into());
            }
            spec.elements[id]
                .children
                .iter()
                .find_map(|id| find(spec, id))
        }
        find(self, &self.root)
    }

    pub fn control_key(&mut self, id: &str, key: crossterm::event::KeyCode) -> Result<bool> {
        use crossterm::event::KeyCode::*;
        let widget = self.widget(id)?;
        if let Widget::Select {
            options,
            value,
            searchable: true,
            ..
        } = &widget
        {
            let interaction = self.selections.entry(id.into()).or_default();
            let (handled, next) =
                interaction.key(options, std::slice::from_ref(value), true, false, key);
            if let Some(next) = next {
                self.set_selection(id, json!(next[0]))?;
            }
            return Ok(handled);
        }
        if let Widget::MultiSelect {
            options,
            value,
            searchable,
            ..
        } = &widget
        {
            let interaction = self.selections.entry(id.into()).or_default();
            let (handled, next) = interaction.key(options, value, *searchable, true, key);
            if let Some(next) = next {
                self.set_selection(id, json!(next))?;
            }
            return Ok(handled);
        }
        let next = match &widget {
            Widget::Select { options, value, .. } => {
                let index = options.iter().position(|v| v == value).unwrap_or(0);
                let next = match key {
                    Up | Left => index.saturating_sub(1),
                    Down | Right => (index + 1).min(options.len() - 1),
                    Home => 0,
                    End => options.len() - 1,
                    Enter | Char(' ') => index,
                    _ => return Ok(false),
                };
                Some(json!(options[next]))
            }
            Widget::Tabs { tabs, value, .. } if matches!(key, Left | Right | Home | End) => {
                let index = tabs.iter().position(|v| &v.label == value).unwrap_or(0);
                let next = match key {
                    Left => (index + tabs.len() - 1) % tabs.len(),
                    Right => (index + 1) % tabs.len(),
                    Home => 0,
                    _ => tabs.len() - 1,
                };
                self.scroll_states.borrow_mut().remove(id);
                Some(json!(tabs[next].label))
            }
            Widget::Slider {
                value,
                min,
                max,
                step,
                ..
            } => {
                let next = match key {
                    Left | Down | Char('-') => value - step,
                    Right | Up | Char('+') => value + step,
                    Home => *min,
                    End => *max,
                    _ => return Ok(false),
                };
                Some(json!((next.clamp(*min, *max) * 1e8).round() / 1e8))
            }
            Widget::ScrollView { .. } | Widget::Tabs { .. } => {
                let mut states = self.scroll_states.borrow_mut();
                let state = states.entry(id.into()).or_default();
                match key {
                    Up => state.scroll_up(),
                    Down => state.scroll_down(),
                    Left => state.scroll_left(),
                    Right => state.scroll_right(),
                    PageDown => state.scroll_page_down(),
                    PageUp => state.scroll_page_up(),
                    Home => state.scroll_to_top(),
                    End => state.scroll_to_bottom(),
                    _ => return Ok(false),
                }
                return Ok(true);
            }
            _ => return Ok(false),
        };
        if let Some(value) = next {
            let path = self.elements[id].props["value"]["$bindState"]
                .as_str()
                .unwrap()
                .to_owned();
            self.update(&path, value)?;
        }
        Ok(true)
    }

    fn set_selection(&mut self, id: &str, value: Value) -> Result<()> {
        let path = self.elements[id].props["value"]["$bindState"]
            .as_str()
            .ok_or("Missing selection binding")?
            .to_owned();
        self.update(&path, value)
    }

    fn sizing(&self, id: &str) -> Sizing {
        Sizing::take(&mut resolve(&self.elements[id].props, &self.state).expect("validated props"))
            .expect("validated sizing")
    }

    fn sized_area(&self, id: &str, area: Rect) -> Rect {
        let sizing = self.sizing(id);
        let content = if matches!(sizing.width, Some(Width::Mode(WidthMode::Content))) {
            self.content_width(id, area.width)
        } else {
            0
        };
        sizing.area(area, content)
    }

    fn container_layout(&self, id: &str, width: u16) -> NodeLayout<'_> {
        let children: Vec<_> = self.elements[id]
            .children
            .iter()
            .filter(|id| self.is_visible(id))
            .map(String::as_str)
            .collect();
        let (columns, gap, row_gap) = match self.widget(id).expect("validated spec") {
            Widget::Row {
                gap,
                collapse_below,
            } if width >= collapse_below => (children.len().max(1), gap, 0),
            Widget::Grid {
                columns,
                gap,
                min_cell_width,
            } => {
                let space = gap.unwrap_or(1);
                let count = if min_cell_width == 0 {
                    columns
                } else {
                    columns
                        .min(width.saturating_add(space) / min_cell_width.saturating_add(space))
                        .max(1)
                };
                (count as usize, space, gap.unwrap_or(0))
            }
            Widget::Column { gap } | Widget::Panel { gap, .. } | Widget::Row { gap, .. } => {
                (1, 0, gap)
            }
            _ => (1, 0, 0),
        };
        NodeLayout {
            rows: children.chunks(columns).map(|r| r.to_vec()).collect(),
            columns,
            gap,
            row_gap,
        }
    }

    fn inner_area(&self, id: &str, area: Rect) -> Rect {
        if let Widget::Panel {
            title,
            border,
            padding,
            title_align,
            ..
        } = self.widget(id).expect("validated spec")
        {
            widgets::panel_block(&title, &border, padding, &title_align).inner(area)
        } else {
            area
        }
    }

    fn row_cells(&self, id: &str, layout: &NodeLayout<'_>, area: Rect) -> std::rc::Rc<[Rect]> {
        let constraints = if matches!(self.widget(id), Ok(Widget::Row { collapse_below, .. }) if area.width >= collapse_below)
        {
            layout
                .rows
                .iter()
                .flatten()
                .map(|child| {
                    let sizing = self.sizing(child);
                    let cap = if sizing.max_width == 0 {
                        area.width
                    } else {
                        sizing.max_width.min(area.width)
                    };
                    match sizing.width {
                        Some(Width::Cells(width)) => Constraint::Length(width.min(cap)),
                        Some(Width::Mode(WidthMode::Content)) => {
                            Constraint::Length(self.content_width(child, area.width).min(cap))
                        }
                        _ if sizing.max_width > 0 => Constraint::Max(cap),
                        _ => Constraint::Fill(1),
                    }
                })
                .collect()
        } else {
            vec![Constraint::Fill(1); layout.columns]
        };
        Layout::horizontal(constraints)
            .spacing(layout.gap)
            .split(area)
    }

    fn height(&self, id: &str, width: u16) -> u16 {
        if !self.is_visible(id) {
            return 0;
        }
        let width = self.sized_area(id, Rect::new(0, 0, width, 1)).width;
        match self.widget(id).expect("validated spec") {
            Widget::Column { .. }
            | Widget::Row { .. }
            | Widget::Grid { .. }
            | Widget::Panel { .. } => {
                let inner = self.inner_area(id, Rect::new(0, 0, width, 4096));
                let layout = self.container_layout(id, inner.width);
                let cells = self.row_cells(id, &layout, Rect::new(0, 0, inner.width, 1));
                let gaps = layout
                    .row_gap
                    .saturating_mul(layout.rows.len().saturating_sub(1) as u16);
                layout
                    .rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .zip(cells.iter())
                            .map(|(child, cell)| self.height(child, cell.width))
                            .max()
                            .unwrap_or(0)
                    })
                    .fold(
                        (4096 - inner.height).saturating_add(gaps),
                        u16::saturating_add,
                    )
            }
            Widget::Text { text, .. } => (text.lines().count().max(1) as u16).saturating_add(1),
            Widget::Table(table) => table.height(),
            Widget::List { items, .. } => items.len() as u16 + 2,
            Widget::Keypad { mode, .. } => mode.height(),
            Widget::Badge { .. } | Widget::Switch { .. } => 2,
            Widget::Sparkline { dense: true, .. } => 8,
            other => other.extra_height(),
        }
    }

    pub fn content_height(&self) -> u16 {
        self.content_height_in(100)
    }
    pub fn content_height_in(&self, width: u16) -> u16 {
        self.height(&self.root, width)
    }

    fn grow(&self, id: &str) -> u16 {
        let element = &self.elements[id];
        if let Some(grow) = resolve(&element.props, &self.state)
            .ok()
            .and_then(|p| p["grow"].as_u64())
        {
            return grow as u16;
        }
        // Containers carry their children's expansion through existing wrappers.
        // Explicit grow:0 stops inheritance for a fixed-height section.
        element
            .children
            .iter()
            .filter(|id| self.is_visible(id))
            .map(|id| self.grow(id))
            .max()
            .unwrap_or(0)
    }

    /// Share only surplus height, keeping the intrinsic minimum of every child.
    fn vertical_areas(area: Rect, sizes: &[(u16, u16)], gap: u16) -> std::rc::Rc<[Rect]> {
        let minimum: u32 = sizes.iter().map(|(height, _)| u32::from(*height)).sum();
        let mut extra = u32::from(area.height)
            .saturating_sub(minimum + u32::from(gap) * sizes.len().saturating_sub(1) as u32);
        let mut weight: u32 = sizes.iter().map(|(_, grow)| u32::from(*grow)).sum();
        let constraints: Vec<_> = sizes
            .iter()
            .map(|&(height, grow)| {
                let share = if weight == 0 {
                    0
                } else {
                    extra * u32::from(grow) / weight
                };
                extra -= share;
                weight -= u32::from(grow);
                Constraint::Length(height.saturating_add(share as u16))
            })
            .collect();
        Layout::vertical(constraints).spacing(gap).split(area)
    }

    // Rendering and focus navigation use the exact same allocation.
    fn child_areas<'a>(&'a self, id: &str, area: Rect) -> Vec<(&'a str, Rect)> {
        let inner = self.inner_area(id, area);
        let layout = self.container_layout(id, inner.width);
        let cells = self.row_cells(id, &layout, Rect::new(0, 0, inner.width, 1));
        let sizes: Vec<_> = layout
            .rows
            .iter()
            .map(|row| {
                (
                    row.iter()
                        .zip(cells.iter())
                        .map(|(child, cell)| self.height(child, cell.width))
                        .max()
                        .unwrap_or(0),
                    row.iter().map(|id| self.grow(id)).max().unwrap_or(0),
                )
            })
            .collect();
        // Horizontal groups receive the full allocated height, even without growing leaves.
        let areas = if layout.rows.len() == 1
            && matches!(self.widget(id),Ok(Widget::Row {collapse_below,..}) if inner.width>=collapse_below)
        {
            vec![inner]
        } else {
            Self::vertical_areas(inner, &sizes, layout.row_gap).to_vec()
        };
        layout
            .rows
            .iter()
            .zip(areas.iter())
            .flat_map(|(row, area)| {
                let cells = self.row_cells(id, &layout, *area);
                row.iter()
                    .zip(cells.iter())
                    .map(|(id, area)| (*id, *area))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn focus_offset(&self, target: &str) -> u16 {
        self.focus_offset_in(target, Rect::new(0, 0, 100, self.content_height()))
    }

    pub fn focus_offset_in(&self, target: &str, viewport: Rect) -> u16 {
        fn find(spec: &Spec, id: &str, target: &str, area: Rect) -> Option<u16> {
            if !spec.is_visible(id) {
                return None;
            }
            let area = spec.sized_area(id, area);
            if id == target {
                return Some(area.y);
            }
            spec.child_areas(id, area)
                .into_iter()
                .find_map(|(child, area)| find(spec, child, target, area))
        }
        let area = Rect::new(
            0,
            0,
            viewport.width,
            self.content_height_in(viewport.width)
                .max(viewport.height)
                .min(4096),
        );
        find(self, &self.root, target, area).unwrap_or(0)
    }

    /// Draw a validated spec. The application owns scroll, focus and action dispatch.
    pub fn render(&self, frame: &mut Frame, area: Rect, focus: Option<&str>, scroll: u16) {
        if area.is_empty() {
            return;
        }
        let height = self
            .content_height_in(area.width)
            .max(area.height)
            .min(4096);
        // ponytail: buffer up to 4,096 rows; virtualize widgets if larger specs are needed.
        // The separate cell buffer clips scrolling at the viewport.
        let backend = ratatui::backend::TestBackend::new(area.width, height);
        let mut terminal = Terminal::new(backend).expect("in-memory terminal");
        terminal
            .draw(|f| {
                f.render_widget(
                    Block::default().style(
                        Style::default()
                            .bg(self.theme.background)
                            .fg(self.theme.text_on(self.theme.background)),
                    ),
                    f.area(),
                );
                self.render_node(f, f.area(), &self.root, focus);
            })
            .expect("in-memory draw");
        let source = terminal.backend().buffer();
        for y in 0..area.height {
            for x in 0..area.width {
                if let Some(cell) = source.cell((x, y.saturating_add(scroll))) {
                    let mut cell = cell.clone();
                    if cell.fg == Color::Reset {
                        cell.fg = self.theme.text_on(self.theme.background);
                    }
                    if cell.bg == Color::Reset {
                        cell.bg = self.theme.background;
                    }
                    frame.buffer_mut()[(area.x + x, area.y + y)] = cell;
                }
            }
        }
        if let Some(id) = self.active_popup()
            && let Ok(Widget::Popup { title, body, .. }) = self.widget(&id)
        {
            frame.render_widget(
                tui_widgets::popup::Popup::new(Text::from(format!(
                    "{}\n\nEnter or Esc to close",
                    clean(&body)
                )))
                .title(clean(&title))
                .style(
                    Style::default()
                        .fg(self.theme.foreground)
                        .bg(self.theme.background),
                ),
                area,
            );
        }
    }

    fn render_node(&self, frame: &mut Frame, area: Rect, id: &str, focus: Option<&str>) {
        if area.is_empty() || !self.is_visible(id) {
            return;
        }
        let area = self.sized_area(id, area);
        let widget = self.widget(id).expect("validated spec");
        let theme = &self.theme;
        let accent = theme.primary;
        let block = |title: String| {
            theme
                .block(BorderType::Plain)
                .title(title)
                .border_style(Style::default().fg(theme.border))
        };
        match widget {
            Widget::Column { .. }
            | Widget::Row { .. }
            | Widget::Grid { .. }
            | Widget::Panel { .. } => {
                if let Widget::Panel {
                    surface,
                    title,
                    border,
                    padding,
                    title_align,
                    ..
                } = &widget
                {
                    frame.render_widget(
                        widgets::panel_block(title, border, *padding, title_align)
                            .style(if *surface {
                                Style::default().bg(theme.surface)
                            } else {
                                Style::default()
                            })
                            .title_style(theme.heading())
                            .border_style(Style::default().fg(theme.border)),
                        area,
                    );
                }
                for (child, area) in self.child_areas(id, area) {
                    self.render_node(frame, area, child, focus);
                }
            }
            Widget::Text { text, muted } => frame.render_widget(
                Paragraph::new(clean(&text)).style(Style::default().fg(if muted {
                    theme.muted
                } else {
                    theme.text_on(theme.background)
                })),
                area,
            ),
            Widget::Metric {
                label,
                value,
                color,
                thresholds,
            } => frame.render_widget(
                Paragraph::new(vec![
                    Line::from(clean(&label)).style(Style::default().fg(theme.muted)),
                    Line::from(clean(&value)).style(
                        if let Some(color) =
                            theme.value_color(&color, &thresholds, theme::numeric_value(&value))
                        {
                            Style::default().fg(color).bold()
                        } else if theme.design.is_some() {
                            theme.heading()
                        } else {
                            Style::default().fg(accent).bold()
                        },
                    ),
                ]),
                area,
            ),
            Widget::Gauge {
                title,
                value,
                color,
                thresholds,
            } => frame.render_widget(
                Gauge::default()
                    .block(block(clean(&title)))
                    .ratio(value / 100.0)
                    .label(format!("{value:.0}%"))
                    .gauge_style(
                        Style::default().fg(theme
                            .value_color(&color, &thresholds, Some(value))
                            .unwrap_or(accent)),
                    ),
                area,
            ),
            Widget::Sparkline {
                title,
                data,
                dense: false,
                color,
                thresholds,
            } => frame.render_widget(
                Sparkline::default()
                    .block(block(clean(&title)))
                    .style(
                        Style::default().fg(theme
                            .value_color(&color, &thresholds, data.last().map(|v| *v as f64))
                            .unwrap_or(accent)),
                    )
                    .data(data),
                area,
            ),
            Widget::Sparkline {
                title,
                data,
                dense: true,
                color,
                thresholds,
            } => {
                let panel = block(clean(&title));
                let inner = panel.inner(area);
                frame.render_widget(panel, area);
                let color = theme.value_color(&color, &thresholds, data.last().map(|v| *v as f64));
                let max = data.iter().copied().max().unwrap_or(1).max(1) as f64;
                frame.render_widget(
                    tui_widgets::bar_graph::BarGraph::new(
                        data.into_iter().map(|v| v as f64).collect(),
                    )
                    .with_min(0.0)
                    .with_max(max)
                    .with_bar_style(tui_widgets::bar_graph::BarStyle::Braille),
                    inner,
                );
                if let Some(color) = color {
                    frame
                        .buffer_mut()
                        .set_style(inner, Style::default().fg(color));
                }
            }
            Widget::Table(table) => table.render(theme, frame, area),
            Widget::Select {
                title,
                options,
                value,
                searchable: true,
                bordered,
            } => {
                self.selections.get(id).cloned().unwrap_or_default().render(
                    theme,
                    frame,
                    area,
                    &title,
                    &options,
                    &[value],
                    true,
                    false,
                    focus == Some(id),
                    bordered,
                );
            }
            Widget::MultiSelect {
                title,
                options,
                value,
                searchable,
                bordered,
            } => {
                self.selections.get(id).cloned().unwrap_or_default().render(
                    theme,
                    frame,
                    area,
                    &title,
                    &options,
                    &value,
                    searchable,
                    true,
                    focus == Some(id),
                    bordered,
                );
            }
            Widget::List { title, items } => frame.render_widget(
                List::new(items.into_iter().map(|v| clean(&v))).block(block(clean(&title))),
                area,
            ),
            Widget::Input { label, value } => {
                let selected = focus == Some(id);
                frame.render_widget(
                    Paragraph::new(format!(
                        "{}{}",
                        clean(&value),
                        if selected { "▏" } else { "" }
                    ))
                    .block(block(clean(&label)).border_style(
                        Style::default().fg(if selected { theme.focus } else { theme.border }),
                    )),
                    area,
                );
            }
            Widget::Button {
                label,
                hotkey,
                intent,
                disabled,
                variant,
            } => controls::button(
                theme,
                frame,
                area,
                &clean(&controls::button_label(&label, &hotkey)),
                intent,
                focus == Some(id),
                disabled,
                variant,
            ),
            Widget::Badge { label, intent } => frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {} ", clean(&label)),
                    Style::default()
                        .fg(intent.foreground(theme))
                        .bg(intent.color(theme))
                        .bold(),
                ))),
                area,
            ),
            Widget::Switch {
                label,
                checked,
                disabled,
            } => frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        if checked {
                            "[─●] On  "
                        } else {
                            "[●─] Off "
                        },
                        Style::default().fg(if disabled {
                            self.theme.muted
                        } else {
                            self.theme.primary
                        }),
                    ),
                    Span::raw(clean(&label)),
                ]))
                .style(if focus == Some(id) {
                    Style::default()
                        .bg(self.theme.surface)
                        .fg(self.theme.foreground)
                        .bold()
                } else {
                    Style::default()
                }),
                area,
            ),
            Widget::Keypad { title, value, mode } => controls::keypad(
                theme,
                frame,
                area,
                &clean(&title),
                &clean(&value),
                mode,
                (focus == Some(id)).then(|| self.key_selection.get(id).copied().unwrap_or(0)),
            ),
            other => other.render_extra(
                theme,
                frame,
                area,
                focus == Some(id),
                self.scroll_states
                    .borrow_mut()
                    .entry(id.into())
                    .or_default(),
            ),
        }
    }
}

fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(4096)
        .collect()
}
