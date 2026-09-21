use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use generated_tui_runtime::Spec;
use ratatui::{prelude::*, widgets::Paragraph};
use std::time::Duration;
mod actions;
mod ui;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    for arg in &args {
        if !["--check", "--snapshot", "--help"].contains(&arg.as_str()) {
            return Err(format!("Unknown option: {arg}").into());
        }
    }
    if args.iter().any(|a| a == "--help") {
        println!("Terminal app [--check | --snapshot] · Tab select · Enter activate · Ctrl+C quit");
        return Ok(());
    }
    let spec = ui::build()?;
    spec.validate()?;
    if args.iter().any(|a| a == "--snapshot") {
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(100, spec.content_height().clamp(1, 4096)))?;
        terminal.draw(|f| spec.render(f, f.area(), None, 0))?;
        let buffer = terminal.backend().buffer();
        for y in 0..buffer.area.height {
            let line: String = (0..buffer.area.width).map(|x| buffer[(x,y)].symbol()).collect();
            println!("{}", line.trim_end());
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--check") {
        println!("ok");
        return Ok(());
    }
    let mut terminal = ratatui::init();
    let result = crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))
        .map_err(Into::into).and_then(|_| run(&mut terminal, spec));
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, mut spec: Spec) -> Result<()> {
    let mut focus = spec.focusable().first().cloned();
    let mut scroll = 0u16;
    let mut status = String::from("Tab / Shift+Tab select · Enter activate · PgUp/PgDn scroll · Ctrl+C quit");
    loop {
        let ids = spec.focusable();
        if focus.as_ref().is_none_or(|id| !ids.contains(id)) { focus = ids.first().cloned(); }
        if let Some(id) = spec.active_popup() { focus = Some(id); }
        let viewport = terminal.size()?.height.saturating_sub(1);
        scroll = scroll.min(spec.content_height_in(terminal.size()?.width).saturating_sub(viewport));
        terminal.draw(|frame| {
            let areas = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
            spec.render(frame, areas[0], focus.as_deref(), scroll);
            frame.render_widget(Paragraph::new(status.chars().filter(|c| !c.is_control()).collect::<String>()).style(Style::default().fg(Color::Gray)), areas[1]);
        })?;
        if !event::poll(Duration::from_millis(100))? { continue; }
        let Event::Key(mut key) = event::read()? else { continue };
        if key.kind == KeyEventKind::Release { continue; }
        let control = key.modifiers == KeyModifiers::CONTROL;
        if control && key.code == KeyCode::Char('c') { return Ok(()); }
        if key.code == KeyCode::Esc {
            if let Some(id) = spec.active_popup() {
                if let Err(error) = spec.activate(&id) { status = error; }
            }
            continue;
        }
        if let Some(id) = spec.hotkey_target(key) {
            focus = Some(id.clone());
            scroll = spec.focus_offset_in(&id, Rect::new(0, 0, terminal.size()?.width, viewport)).saturating_sub(viewport.saturating_sub(3));
            key.code = KeyCode::Enter;
            key.modifiers = KeyModifiers::NONE;
        } else if key.modifiers.intersects(KeyModifiers::ALT | KeyModifiers::CONTROL) {
            continue;
        }
        if let Some(id) = focus.as_ref() {
            match spec.control_key(id, key.code) {
                Ok(true) => continue,
                Err(error) => { status = error; continue; }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                if !ids.is_empty() {
                    let index = focus.as_ref().and_then(|id| ids.iter().position(|s| s == id)).unwrap_or(0);
                    let index = if key.code == KeyCode::BackTab { (index + ids.len() - 1) % ids.len() } else { (index + 1) % ids.len() };
                    focus = Some(ids[index].clone());
                    scroll = spec.focus_offset_in(&ids[index], Rect::new(0, 0, terminal.size()?.width, viewport)).saturating_sub(viewport.saturating_sub(3));
                }
                continue;
            }
            KeyCode::PageDown => { scroll = scroll.saturating_add(viewport.max(1)); continue; }
            KeyCode::PageUp => { scroll = scroll.saturating_sub(viewport.max(1)); continue; }
            _ => {}
        }
        let Some(id) = focus.as_ref() else { continue };
        let kind = spec.elements[id].kind.clone();
        let result = if kind == "Keypad" {
            match key.code {
                KeyCode::Left => spec.keypad_move(id, -1, 0),
                KeyCode::Right => spec.keypad_move(id, 1, 0),
                KeyCode::Up => spec.keypad_move(id, 0, -1),
                KeyCode::Down => spec.keypad_move(id, 0, 1),
                KeyCode::Enter | KeyCode::Char(' ') => spec.activate(id),
                KeyCode::Backspace => spec.keypad_type(id, "Backspace"),
                KeyCode::Delete => spec.keypad_type(id, "AC"),
                KeyCode::Char(c) if !control => spec.keypad_type(id, &c.to_string()),
                _ => continue,
            }
        } else if kind == "Input" {
            match key.code {
                KeyCode::Char(c) if !control => spec.edit_input(id, Some(c)),
                KeyCode::Backspace => spec.edit_input(id, None),
                _ => continue,
            }
        } else {
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let previous = spec.clone();
                    let result = if kind == "Button" {
                        actions::handle(&mut spec, id).and_then(|handled| if handled { spec.validate() } else { spec.activate(id) })
                    } else { spec.activate(id) };
                    if result.is_err() { spec = previous; }
                    result
                }
                KeyCode::Down => { scroll = scroll.saturating_add(1); continue; }
                KeyCode::Up => { scroll = scroll.saturating_sub(1); continue; }
                _ => continue,
            }
        };
        status = result.map(|_| "Local action applied".into()).unwrap_or_else(|e| e);
    }
}
