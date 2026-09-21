use crate::{Result, edit_prompt, project::Project};
use crossterm::event::KeyCode;
use ratatui::{prelude::*, widgets::*};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    New,
    Open,
    Help,
    Delete,
}

pub struct Navigation {
    pub page: Page,
    pub input: String,
    pub cursor: usize,
    pub base: PathBuf,
    recent: Vec<PathBuf>,
    selected: Option<usize>,
}

impl Navigation {
    pub fn new(page: Page, project: &Project, input: &str) -> Self {
        Self {
            page,
            input: input.into(),
            cursor: input.len(),
            base: project.dir.parent().unwrap_or(Path::new(".")).into(),
            recent: project.recent_projects(),
            selected: None,
        }
    }

    pub fn location(&self) -> Result<PathBuf> {
        let input = self.input.trim();
        if input.is_empty() {
            return Err(
                "Enter a project folder or choose a recent project with the arrow keys.".into(),
            );
        }
        let path = if input == "~" || input.starts_with("~/") {
            PathBuf::from(std::env::var_os("HOME").ok_or("Home folder is unavailable")?)
                .join(input.strip_prefix("~/").unwrap_or(""))
        } else {
            PathBuf::from(input)
        };
        Ok(if path.is_absolute() {
            path
        } else {
            self.base.join(path)
        })
    }

    pub fn edit(&mut self, key: KeyCode) {
        if matches!(self.page, Page::Open | Page::Delete)
            && matches!(key, KeyCode::Up | KeyCode::Down)
            && !self.recent.is_empty()
        {
            let count = self.recent.len();
            let index = match self.selected {
                None => 0,
                Some(i) if key == KeyCode::Up => (i + count - 1) % count,
                Some(i) => (i + 1) % count,
            };
            self.selected = Some(index);
            self.input = self.recent[index].to_string_lossy().into();
            self.cursor = self.input.len();
        } else {
            self.selected = None;
            edit_prompt(&mut self.input, &mut self.cursor, key);
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let area = area.inner(Margin::new(2, 1));
        let title = match self.page {
            Page::New => "New project",
            Page::Open => "Open project",
            Page::Help => "Project controls",
            Page::Delete => "Delete project",
        };
        let mut lines = vec![
            Line::styled(title, Style::default().bold().fg(Color::Cyan)),
            Line::raw(""),
        ];
        if self.page == Page::Help {
            for text in [
                "Ctrl+N   New project in a new or empty folder",
                "Ctrl+O   Open a saved project; choose a recent one or type its folder",
                "Ctrl+S   Save preview changes and generated Rust source",
                "Ctrl+Z   Restore a changed preview; otherwise undo the saved version",
                "",
                "Each successful build or edit saves automatically.",
                "Preview changes also save before you switch projects or quit.",
                "The project folder contains source, ui.json and version history.",
                "",
                "Enter builds or applies feedback. Tab tries the preview.",
                "Ctrl+G toggles chat. Esc hides it; Ctrl+C quits.",
                "Ctrl+P opens the project menu. Ctrl+X cancels a build.",
                "",
                "/new [folder]  /open [folder]  /save  /undo  /export  /help",
            ] {
                lines.push(Line::raw(text));
            }
        } else {
            lines.push(Line::raw(if self.page == Page::New {
                "Choose a name or folder. Your current project stays saved."
            } else {
                "Type a folder, or use ↑ / ↓ to choose a saved project."
            }));
            lines.push(Line::raw(format!(
                "Relative folders start in {}",
                self.base.display()
            )));
            lines.push(Line::raw("Enter confirms · Esc goes back"));
            if matches!(self.page, Page::Open | Page::Delete) {
                lines.extend([
                    Line::raw(""),
                    Line::styled("Recent projects", Style::default().bold()),
                ]);
                if self.recent.is_empty() {
                    lines.push(Line::raw(
                        "No saved projects yet. Enter an existing project folder below.",
                    ));
                }
                let available = usize::from(area.height.saturating_sub(8)).max(1);
                let start = self
                    .selected
                    .unwrap_or(0)
                    .saturating_sub(available.saturating_sub(1));
                for (i, path) in self.recent.iter().enumerate().skip(start).take(available) {
                    lines.push(Line::styled(
                        format!(
                            "{} {}  ·  {}",
                            if self.selected == Some(i) { "›" } else { " " },
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            path.display()
                        ),
                        if self.selected == Some(i) {
                            Style::default().fg(Color::Cyan).bold()
                        } else {
                            Style::default()
                        },
                    ));
                }
            }
        }
        let lines: Vec<Line> = lines
            .into_iter()
            .map(|line| {
                Line::from(
                    line.spans
                        .into_iter()
                        .map(|span| {
                            Span::styled(
                                span.content
                                    .chars()
                                    .filter(|c| !c.is_control())
                                    .collect::<String>(),
                                span.style,
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), area);
    }
}

pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

pub fn host_theme() -> ratatui_json::theme::Theme {
    static THEME: std::sync::OnceLock<ratatui_json::theme::Theme> = std::sync::OnceLock::new();
    *THEME.get_or_init(|| {
        ratatui_json::Spec::parse(include_str!("../ui/launcher.json"))
            .expect("embedded launcher validated")
            .theme
    })
}

// Generated by TUI Draw, then bound to local project data. See ui/README.md.
fn home_spec(paths: &[PathBuf], selected: usize, status: &str) -> Result<ratatui_json::Spec> {
    use serde_json::json;
    let mut spec = ratatui_json::Spec::parse(include_str!("../ui/launcher.json"))?;
    let mut labels = Vec::new();
    for path in paths {
        let name: String =
            crate::clean_text(&path.file_name().unwrap_or_default().to_string_lossy())
                .chars()
                .take(60)
                .collect();
        let mut label = name.clone();
        let mut suffix = 2;
        while labels.contains(&label) {
            label = format!("{name} ({suffix})");
            suffix += 1;
        }
        labels.push(label);
    }
    let labels = if labels.is_empty() {
        vec!["No saved projects yet".into()]
    } else {
        labels
    };
    spec.state["selectedProject"] = json!(labels[selected.min(labels.len() - 1)]);
    spec.elements.get_mut("projects").unwrap().props["options"] = json!(labels);
    for id in ["open", "delete"] {
        spec.elements.get_mut(id).unwrap().props["disabled"] = json!(paths.is_empty());
    }
    set_home_status(&mut spec, status);
    set_project_path(&mut spec, paths);
    spec.validate()?;
    Ok(spec)
}

fn set_home_status(spec: &mut ratatui_json::Spec, status: &str) {
    spec.state["status"] = serde_json::json!(crate::clean_text(status));
    spec.state["hasStatus"] = serde_json::json!(!status.is_empty());
}

fn update_hint(spec: &mut ratatui_json::Spec, focus: &str, page: Option<Page>, can_return: bool) {
    let action = if focus == "projects" {
        if spec.elements["open"].props["disabled"] == true {
            "".into()
        } else {
            " · Enter open".to_owned()
        }
    } else if focus == "folder" {
        format!(
            " · Enter {}",
            if page == Some(Page::New) {
                "create"
            } else {
                "open"
            }
        )
    } else {
        spec.elements
            .get(focus)
            .and_then(|e| e.props["label"].as_str())
            .map(|label| format!(" · Enter {}", label.trim_end_matches('…').to_lowercase()))
            .unwrap_or_default()
    };
    let navigation = if focus == "projects" {
        "↑ ↓ choose · Tab actions"
    } else {
        "Tab next"
    };
    let exit = if page.is_some() {
        " · Esc cancel"
    } else if can_return {
        " · Esc back"
    } else {
        " · Ctrl+C quit"
    };
    spec.elements.get_mut("instruction").unwrap().props["text"] =
        serde_json::json!(format!("{navigation}{action}{exit}"));
}

fn selected_index(spec: &ratatui_json::Spec) -> usize {
    spec.elements["projects"].props["options"]
        .as_array()
        .unwrap()
        .iter()
        .position(|label| label == &spec.state["selectedProject"])
        .unwrap_or(0)
}

fn set_project_path(spec: &mut ratatui_json::Spec, paths: &[PathBuf]) {
    spec.state["projectPath"] = serde_json::json!(
        paths
            .get(selected_index(spec))
            .map(|p| crate::clean_text(&p.display().to_string()))
            .unwrap_or_else(|| "Choose New project or Add existing to get started.".into())
    );
}

fn dialog_spec(
    theme: ratatui_json::theme::Theme,
    page: Page,
    path: &Path,
) -> Result<ratatui_json::Spec> {
    use serde_json::json;
    let (title, body, label, command) = match page {
        Page::Delete => (
            "Delete project",
            format!(
                "Delete {}?\n\nSource and history will be kept.\nUndo is available from the project menu.",
                crate::clean_text(&path.file_name().unwrap_or_default().to_string_lossy())
            ),
            "Delete project",
            "confirm-delete",
        ),
        Page::New => (
            "New project",
            "Give your project a name or choose an empty folder.".into(),
            "Create project",
            "submit",
        ),
        _ => (
            "Add existing project",
            "Enter the folder containing your saved project.".into(),
            "Open project",
            "submit",
        ),
    };
    let mut raw = json!({"root":"dialog","theme":theme,"state":{"folder":"","command":""},"elements":{
        "dialog":{"type":"Panel","props":{"title":title,"border":"rounded","surface":true,"padding":1,"gap":1},"children":["body","folder","actions","status","instruction"]},
        "body":{"type":"Text","props":{"text":body,"muted":false}},
        "folder":{"type":"Input","props":{"label":"Project folder","value":{"$bindState":"/folder"}}},
        "actions":{"type":"Grid","props":{"columns":2,"gap":2,"minCellWidth":30},"children":["cancel","confirm"]},
        "cancel":{"type":"Button","props":{"label":"Cancel","hotkey":"Alt+C","intent":"neutral"},"on":{"press":{"action":"setState","params":{"statePath":"/command","value":"cancel"}}}},
        "confirm":{"type":"Button","props":{"label":label,"hotkey":if page==Page::Delete {"Alt+Y"} else if page==Page::New {"Alt+N"} else {"Alt+O"},"intent":if page==Page::Delete {"danger"} else {"primary"}},"on":{"press":{"action":"setState","params":{"statePath":"/command","value":command}}}},
        "status":{"type":"Text","props":{"text":"","muted":true},"visible":false},
        "instruction":{"type":"Text","props":{"text":"Tab next · Esc cancel","muted":true}}
    }});
    if page == Page::Delete {
        raw["elements"]["folder"] = json!({"type":"Text","props":{"text":crate::clean_text(&path.display().to_string()),"muted":true}});
    }
    Ok(ratatui_json::Spec::parse(&raw.to_string())?)
}

fn cycle_focus(spec: &ratatui_json::Spec, current: &str, reverse: bool) -> String {
    let ids = spec.focusable();
    let index = ids.iter().position(|id| id == current).unwrap_or(0);
    ids[(index + if reverse { ids.len() - 1 } else { 1 }) % ids.len()].clone()
}

fn node_rect(node: &serde_json::Value) -> Rect {
    let values: Vec<u16> = node["bounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_u64().unwrap() as u16)
        .collect();
    Rect::new(values[0], values[1], values[2], values[3])
}

fn view_area(spec: &ratatui_json::Spec, area: Rect, dialog: bool) -> Rect {
    let width = area.width.min(if dialog { 66 } else { 72 });
    let height = spec
        .content_height_in(width)
        .min(if dialog { 21 } else { 28 });
    centered(area, width, height)
}

#[cfg(test)]
pub fn render_home(
    frame: &mut Frame,
    paths: &[PathBuf],
    selected: usize,
    status: &str,
    _can_return: bool,
) {
    let spec = home_spec(paths, selected, status).expect("embedded launcher validated");
    frame.render_widget(Block::default().bg(spec.theme.background), frame.area());
    spec.render(
        frame,
        view_area(&spec, frame.area(), false),
        Some(if paths.is_empty() { "new" } else { "projects" }),
        0,
    );
}

/// Render the generated UI locally; only button commands cross into filesystem operations.
pub fn choose_project(
    terminal: &mut ratatui::DefaultTerminal,
    anchor: &Project,
    mut can_return: bool,
) -> Result<Option<Project>> {
    use crossterm::event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, KeyModifiers,
        MouseButton, MouseEventKind,
    };
    use serde_json::json;
    crossterm::execute!(std::io::stdout(), EnableMouseCapture)?;
    struct MouseCapture;
    impl Drop for MouseCapture {
        fn drop(&mut self) {
            let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
        }
    }
    let _mouse = MouseCapture;
    let return_to_anchor = can_return;
    let mut undo: Option<(PathBuf, PathBuf)> = None;
    let mut paths = anchor.recent_projects();
    let mut home = home_spec(&paths, 0, "")?;
    let mut view = home.clone();
    let mut page = None;
    let mut deleting = None;
    let mut focus = if paths.is_empty() { "new" } else { "projects" }.to_owned();
    let mut cursor = 0;
    loop {
        update_hint(&mut view, &focus, page, can_return);
        let size = terminal.size()?;
        let area = view_area(
            &view,
            Rect::new(0, 0, size.width, size.height),
            page.is_some(),
        );
        let scroll = view
            .focus_offset_in(&focus, Rect::new(0, 0, area.width, area.height))
            .saturating_sub(area.height.saturating_sub(4))
            .min(
                view.content_height_in(area.width)
                    .saturating_sub(area.height),
            );
        let layout = view.layout_report(area.width, area.height);
        terminal.draw(|frame| {
            frame.render_widget(Block::default().bg(view.theme.background), frame.area());
            view.render(frame, area, Some(&focus), scroll);
            if focus == "folder" && page != Some(Page::Delete) {
                let bounds = node_rect(&layout["nodes"]["folder"]);
                let field = Rect::new(
                    area.x + bounds.x + 1,
                    area.y + bounds.y.saturating_add(1).saturating_sub(scroll),
                    bounds.width.saturating_sub(2),
                    1,
                );
                if bounds.y.saturating_add(1) >= scroll
                    && area.contains((field.x, field.y).into())
                    && field.width > 0
                {
                    let (text, column) = crate::prompt_window(
                        view.state["folder"].as_str().unwrap_or(""),
                        cursor,
                        field.width,
                    );
                    frame.render_widget(Clear, field);
                    frame.render_widget(
                        Paragraph::new(text)
                            .fg(view.theme.foreground)
                            .bg(view.theme.background),
                        field,
                    );
                    frame.set_cursor_position((field.x + column, field.y));
                }
            }
        })?;
        let event = event::read()?;
        let mut command = String::new();
        match event {
            Event::Paste(text) if focus == "folder" && page != Some(Page::Delete) => {
                let mut input = view.state["folder"].as_str().unwrap_or("").to_owned();
                crate::insert_prompt(&mut input, &mut cursor, &text.replace(['\r', '\n'], " "));
                view.state["folder"] = json!(input);
            }
            Event::Mouse(mouse) => {
                if !area.contains((mouse.column, mouse.row).into()) {
                    continue;
                }
                let point = Position::new(mouse.column - area.x, mouse.row - area.y + scroll);
                if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                    if let Some(id) = view
                        .focusable()
                        .into_iter()
                        .find(|id| node_rect(&layout["nodes"][id]).contains(point))
                    {
                        focus = id;
                        if focus == "projects" {
                            let bounds = node_rect(&layout["nodes"]["projects"]);
                            let inner = if view.elements["projects"].props["bordered"]
                                .as_bool()
                                .unwrap_or(true)
                            {
                                bounds.inner(Margin::new(1, 1))
                            } else {
                                bounds
                            };
                            if inner.contains(point) {
                                let selected = selected_index(&view);
                                let start = selected
                                    .saturating_sub(usize::from(inner.height.saturating_sub(1)));
                                let index = start + usize::from(point.y - inner.y);
                                if let Some(label) = view.elements["projects"].props["options"]
                                    .get(index)
                                    .cloned()
                                {
                                    view.state["selectedProject"] = label;
                                }
                            }
                        } else if view.elements[&focus].kind == "Button" {
                            view.activate(&focus)?;
                        }
                    }
                } else if page.is_none()
                    && matches!(
                        mouse.kind,
                        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                    )
                {
                    focus = "projects".into();
                    view.control_key(
                        &focus,
                        if mouse.kind == MouseEventKind::ScrollUp {
                            KeyCode::Up
                        } else {
                            KeyCode::Down
                        },
                    )?;
                }
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let control = key.modifiers == KeyModifiers::CONTROL;
                if control && key.code == KeyCode::Char('c') {
                    return Ok(None);
                }
                if let Some(id) = view.hotkey_target(key) {
                    focus = id;
                    view.activate(&focus)?;
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    continue;
                } else if key.code == KeyCode::Esc {
                    if page.is_some() {
                        command = "cancel".into();
                    } else if can_return {
                        return Ok(Some(Project::load(&anchor.dir)?));
                    }
                } else if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                    focus = cycle_focus(&view, &focus, key.code == KeyCode::BackTab);
                } else if page.is_none()
                    && control
                    && matches!(key.code, KeyCode::Char('n' | 'o' | 'd'))
                {
                    command = match key.code {
                        KeyCode::Char('n') => "new",
                        KeyCode::Char('o') => "browse",
                        _ => "delete",
                    }
                    .into();
                } else if page.is_none() && key.code == KeyCode::Delete {
                    command = "delete".into();
                } else if key.code == KeyCode::Enter {
                    if focus == "projects" {
                        command = "open".into();
                    } else if focus == "folder" {
                        command = "submit".into();
                    } else {
                        view.activate(&focus)?;
                    }
                } else if focus == "folder" {
                    let mut input = view.state["folder"].as_str().unwrap_or("").to_owned();
                    if control && key.code == KeyCode::Char('u') {
                        input.clear();
                        cursor = 0;
                    } else if !control {
                        edit_prompt(&mut input, &mut cursor, key.code);
                    }
                    view.state["folder"] = json!(input);
                } else if matches!(key.code, KeyCode::Left | KeyCode::Right) && focus != "projects"
                {
                    focus = cycle_focus(&view, &focus, key.code == KeyCode::Left);
                } else if key.code == KeyCode::Char(' ') && view.elements[&focus].kind == "Button" {
                    view.activate(&focus)?;
                } else if focus == "projects" {
                    view.control_key(&focus, key.code)?;
                }
            }
            _ => {}
        }
        if command.is_empty() {
            command = view.state["command"].as_str().unwrap_or("").to_owned();
        }
        view.state["command"] = json!("");
        if page.is_none() {
            set_project_path(&mut view, &paths);
            home = view.clone();
        }
        match command.as_str() {
            "cancel" => {
                page = None;
                deleting = None;
                view = home.clone();
                focus = if paths.is_empty() { "new" } else { "projects" }.into();
            }
            "new" | "browse" => {
                page = Some(if command == "new" {
                    Page::New
                } else {
                    Page::Open
                });
                view = dialog_spec(home.theme, page.unwrap(), &anchor.dir)?;
                focus = "folder".into();
                cursor = 0;
            }
            "open" => {
                if let Some(path) = paths.get(selected_index(&home)) {
                    match Project::load(path) {
                        Ok(project) => {
                            project.remember();
                            return Ok(Some(project));
                        }
                        Err(e) => set_home_status(&mut view, &e.to_string()),
                    }
                }
            }
            "delete" => {
                if let Some(path) = paths.get(selected_index(&home)) {
                    deleting = Some(path.clone());
                    page = Some(Page::Delete);
                    view = dialog_spec(home.theme, Page::Delete, path)?;
                    focus = "cancel".into();
                }
            }
            "confirm-delete" => {
                if let Some(path) = &deleting {
                    match Project::trash(path) {
                        Ok(trashed) => {
                            undo = Some((trashed, path.clone()));
                            can_return &= anchor.dir.join(".builder/session.json").is_file();
                            let selected = selected_index(&home);
                            paths = anchor.recent_projects();
                            home = home_spec(
                                &paths,
                                selected,
                                "Project deleted. Source and history kept.",
                            )?;
                            home.state["canUndo"] = json!(true);
                            view = home.clone();
                            page = None;
                            deleting = None;
                            focus = if paths.is_empty() { "new" } else { "projects" }.into();
                        }
                        Err(e) => {
                            let status = view.elements.get_mut("status").unwrap();
                            status.props["text"] = json!(crate::clean_text(&e.to_string()));
                            status.visible = Some(json!(true));
                        }
                    }
                }
            }
            "undo" => {
                if let Some((trashed, original)) = &undo {
                    match Project::restore(trashed, original) {
                        Ok(project) => {
                            project.remember();
                            paths = anchor.recent_projects();
                            let selected =
                                paths.iter().position(|p| p == &project.dir).unwrap_or(0);
                            home = home_spec(&paths, selected, "Project restored.")?;
                            view = home.clone();
                            focus = "projects".into();
                            can_return = return_to_anchor
                                && anchor.dir.join(".builder/session.json").is_file();
                            undo = None;
                        }
                        Err(e) => set_home_status(&mut view, &e.to_string()),
                    }
                }
            }
            "submit" => {
                if let Some(page) = page {
                    let nav =
                        Navigation::new(page, anchor, view.state["folder"].as_str().unwrap_or(""));
                    match nav.location().and_then(|path| {
                        if page == Page::New {
                            Project::create(path)
                        } else {
                            Project::load(path)
                        }
                    }) {
                        Ok(project) => {
                            project.remember();
                            return Ok(Some(project));
                        }
                        Err(e) => {
                            let status = view.elements.get_mut("status").unwrap();
                            status.props["text"] = json!(crate::clean_text(&e.to_string()));
                            status.visible = Some(json!(true));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;

    #[test]
    fn generated_launcher_and_confirmation_share_real_controls() -> Result<()> {
        let paths = vec![
            PathBuf::from("/Projects/Space trader"),
            PathBuf::from("/Projects/System monitor"),
            PathBuf::from("/Projects/Chat client"),
        ];
        let mut home = home_spec(&paths, 0, "")?;
        home.control_key("projects", KeyCode::Down)?;
        assert_eq!(selected_index(&home), 1);
        set_project_path(&mut home, &paths);
        assert_eq!(home.state["projectPath"], "/Projects/System monitor");
        update_hint(&mut home, "delete", None, false);
        assert!(
            home.elements["instruction"].props["text"]
                .as_str()
                .unwrap()
                .contains("Enter delete")
        );
        set_home_status(&mut home, "Project deleted.");
        assert!(
            home.elements["instruction"].props["text"]
                .as_str()
                .unwrap()
                .contains("Enter delete")
        );
        set_home_status(&mut home, "");
        update_hint(&mut home, "projects", None, false);
        let duplicate_paths = vec![
            PathBuf::from("/one/App"),
            PathBuf::from("/two/App"),
            PathBuf::from("/three/App (2)"),
        ];
        assert_eq!(
            home_spec(&duplicate_paths, 0, "")?.elements["projects"].props["options"],
            json!(["App", "App (2)", "App (2) (2)"])
        );
        home.activate("delete")?;
        assert_eq!(home.state["command"], "delete");
        let mut confirm = dialog_spec(home.theme, Page::Delete, &paths[1])?;
        assert_eq!(confirm.focusable(), vec!["cancel", "confirm"]);
        confirm.activate("cancel")?;
        assert_eq!(confirm.state["command"], "cancel");
        assert!(
            !home_spec(&[], 0, "")?
                .focusable()
                .contains(&"delete".into())
        );
        for (name, spec) in [
            ("launcher", home),
            ("delete", confirm),
            (
                "new",
                dialog_spec(
                    ratatui_json::theme::Theme::default(),
                    Page::New,
                    Path::new("/Projects"),
                )?,
            ),
        ] {
            for (width, height) in [(100, 36), (60, 24), (32, 12), (1, 1)] {
                let mut terminal = Terminal::new(TestBackend::new(width, height))?;
                let area = view_area(&spec, Rect::new(0, 0, width, height), name != "launcher");
                terminal.draw(|frame| {
                    frame.render_widget(Block::default().bg(spec.theme.background), frame.area());
                    spec.render(
                        frame,
                        area,
                        Some(if name == "launcher" {
                            "projects"
                        } else {
                            "cancel"
                        }),
                        0,
                    );
                })?;
                if width == 100
                    && let Ok(dir) = std::env::var("TUI_DRAW_SNAPSHOT_DIR")
                {
                    let buffer = terminal.backend().buffer();
                    let rows: Vec<_>=(0..height).map(|y|(0..width).map(|x| {
                        let cell=&buffer[(x,y)];
                        json!({"text":cell.symbol(),"fg":format!("{:?}",cell.fg),"bg":format!("{:?}",cell.bg)})
                    }).collect::<Vec<_>>()).collect();
                    std::fs::create_dir_all(&dir)?;
                    std::fs::write(
                        Path::new(&dir).join(format!("{name}.json")),
                        serde_json::to_vec(&rows)?,
                    )?;
                }
            }
        }
        Ok(())
    }
}
