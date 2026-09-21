use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use ratatui::{prelude::*, widgets::*};
use ratatui_json::Spec;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
mod cli;
mod export;
mod navigation;
mod project;
use navigation::{Navigation, Page};
use project::Project;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Job {
    child: Child,
    events: mpsc::Receiver<Value>,
}

impl Drop for Job {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn generate(mut request: Value) -> Result<Job> {
    let diagnostics_path = request
        .as_object_mut()
        .and_then(|r| r.remove("diagnosticsPath"))
        .and_then(|v| v.as_str().map(std::path::PathBuf::from));
    let mut child = Command::new(std::env::var("RATATUI_JSON_NODE").unwrap_or("node".into()))
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/bridge/build.mjs"))
        .env("RATATUI_JSON_NATIVE", std::env::current_exe()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // A failed stdin write still reaps the child through Job's drop guard.
    let (tx, events) = mpsc::channel();
    let mut stdin = child.stdin.take().ok_or("No child stdin")?;
    let stdout = child.stdout.take().ok_or("No child stdout")?;
    let stderr = child.stderr.take().ok_or("No child stderr")?;
    let job = Job { child, events };
    serde_json::to_writer(&mut stdin, &request)?;
    stdin.flush()?;
    drop(stdin);
    std::thread::spawn(move || {
        let mut completed = false;
        for line in BufReader::new(stdout).lines() {
            let mut event = match line {
                Ok(line) => serde_json::from_str::<Value>(&line).unwrap_or_else(
                    |_| json!({"type":"error","message":"Invalid bridge response"}),
                ),
                Err(e) => json!({"type":"error","message":e.to_string()}),
            };
            if event["type"] == "error"
                && let Some(path) = &diagnostics_path
                && let Some(details) = event.get("diagnostics")
            {
                let saved = serde_json::to_string_pretty(details)
                    .map_err(|e| e.to_string())
                    .and_then(|text| export::write_atomic(path, &text).map_err(|e| e.to_string()));
                if saved.is_ok() {
                    event["message"] = json!(format!(
                        "{} Details saved in .builder/last-failure.json.",
                        event["message"].as_str().unwrap_or("Generation failed.")
                    ));
                }
            }
            completed |= matches!(event["type"].as_str(), Some("complete" | "error"));
            if tx.send(event).is_err() {
                return;
            }
        }
        if !completed {
            let mut error = String::new();
            let _ = stderr.take(4096).read_to_string(&mut error);
            let _ = tx.send(json!({"type":"error","message": if error.is_empty() { "Composition ended without a result" } else { &error }}));
        }
    });
    Ok(job)
}

fn buffer_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() {
    if let Err(error) = start() {
        if std::env::args().any(|a| a == "--json") {
            println!("{}", json!({"ok":false,"error":error.to_string()}));
        } else {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}

fn start() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--measure-layout"] {
        let mut input = String::new();
        std::io::stdin()
            .take(1_000_001)
            .read_to_string(&mut input)?;
        if input.len() > 1_000_000 {
            return Err("Layout request exceeds 1 MB".into());
        }
        let request: Value = serde_json::from_str(&input)?;
        let width = request["viewport"][0]
            .as_u64()
            .filter(|n| (1..=500).contains(n))
            .ok_or("Invalid viewport width (1..500)")? as u16;
        let height = request["viewport"][1]
            .as_u64()
            .filter(|n| (1..=200).contains(n))
            .ok_or("Invalid viewport height (1..200)")? as u16;
        let spec = Spec::parse(&serde_json::to_string(&request["spec"])?)?;
        println!("{}", spec.layout_report(width, height));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", cli::HELP);
        return Ok(());
    }
    if args.first().is_some_and(|s| !s.starts_with('-')) {
        return cli::dispatch(&args);
    }
    let mut path = None;
    let mut snapshot = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--snapshot" => snapshot = true,
            "--spec" => path = Some(args.next().ok_or("--spec requires a path")?),
            _ => return Err(format!("Unknown option: {arg}").into()),
        }
    }
    let source = if let Some(path) = path {
        std::fs::read_to_string(path)?
    } else if snapshot {
        include_str!("../examples/demo.json").into()
    } else {
        serde_json::to_string(&empty_spec())?
    };
    let spec = Spec::parse(&source)?;
    if snapshot {
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(
            100,
            spec.content_height().clamp(1, 4096),
        ))?;
        terminal.draw(|f| spec.render(f, f.area(), None, 0))?;
        println!("{}", buffer_text(terminal.backend().buffer()));
        return Ok(());
    }
    interactive(spec, None, cli::DEFAULT_ENGINE)
}

fn interactive(spec: Spec, project: Option<Project>, engine: &str) -> Result<()> {
    let direct_chat = project.is_some();
    let mut terminal = ratatui::init();
    // Alternate-screen contents can survive a restart; Ratatui's new diff buffers are blank.
    let result = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        EnableBracketedPaste
    )
    .map_err(Into::into)
    .and_then(|_| {
        let project = match project {
            Some(project) => project,
            None => {
                let anchor = Project::open(std::env::current_dir()?.join(".tui-draw-start"))?;
                let Some(project) = navigation::choose_project(&mut terminal, &anchor, false)?
                else {
                    return Ok(());
                };
                project
            }
        };
        let spec = project.current().cloned().unwrap_or(spec);
        let chat_open = direct_chat || project.current().is_none();
        run(&mut terminal, spec, project, engine, chat_open)
    });
    let _ = crossterm::execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn empty_spec() -> Spec {
    Spec::parse(r#"{"root":"root","elements":{"root":{"type":"Column","props":{}}}}"#).unwrap()
}

fn switch_project(project: &mut Project, spec: &Spec, navigation: &Navigation) -> Result<Project> {
    let path = navigation.location()?;
    if navigation.page == Page::Open {
        Project::load(&path)?;
    }
    if project.preview_changed(spec) {
        project.save_preview(spec)?;
    }
    let next = if navigation.page == Page::New {
        Project::create(&path)?
    } else {
        Project::load(&path)?
    };
    next.remember();
    Ok(next)
}

fn insert_prompt(prompt: &mut String, cursor: &mut usize, text: &str) {
    for c in text.chars().filter(|c| !c.is_control()) {
        if prompt.len() + c.len_utf8() > 4000 {
            break;
        }
        prompt.insert(*cursor, c);
        *cursor += c.len_utf8();
    }
}

fn edit_prompt(prompt: &mut String, cursor: &mut usize, key: KeyCode) {
    let previous = prompt[..*cursor]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0);
    let next = prompt[*cursor..]
        .chars()
        .next()
        .map(|c| *cursor + c.len_utf8())
        .unwrap_or(*cursor);
    match key {
        KeyCode::Char(c) => insert_prompt(prompt, cursor, &c.to_string()),
        KeyCode::Backspace if *cursor > 0 => {
            prompt.drain(previous..*cursor);
            *cursor = previous;
        }
        KeyCode::Delete if *cursor < prompt.len() => {
            prompt.drain(*cursor..next);
        }
        KeyCode::Left => *cursor = previous,
        KeyCode::Right => *cursor = next,
        KeyCode::Home => *cursor = 0,
        KeyCode::End => *cursor = prompt.len(),
        _ => {}
    }
}

fn prompt_window(prompt: &str, cursor: usize, width: u16) -> (&str, u16) {
    let mut start = cursor;
    let mut column = 0;
    for (index, c) in prompt[..cursor].char_indices().rev() {
        let cells = Line::from(c.to_string()).width() as u16;
        if column + cells > width.saturating_sub(1) {
            break;
        }
        column += cells;
        start = index;
    }
    (&prompt[start..], column)
}

fn clean_text(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

fn chat_lines(text: &str, width: u16, style: Style) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut cells = 0;
    for c in clean_text(text).chars() {
        let size = Line::from(c.to_string()).width() as u16;
        if cells + size > width.max(1) && !line.is_empty() {
            lines.push(Line::styled(std::mem::take(&mut line), style));
            cells = 0;
        }
        line.push(c);
        cells += size;
    }
    lines.push(Line::styled(line, style));
    lines
}

#[derive(Default)]
struct LiveChat {
    prompt: String,
    reply: String,
    queued: std::collections::VecDeque<String>,
}

fn render_chat(
    frame: &mut Frame,
    project: &Project,
    input: (&str, usize),
    activity: (&str, &LiveChat, bool),
    offset: u16,
    canvas: Rect,
) -> u16 {
    let (status, live, building) = activity;
    let theme = navigation::host_theme();
    let width = canvas
        .width
        .saturating_sub(4)
        .clamp(1, 110)
        .min(canvas.width);
    let height = canvas.height.min(17);
    let area = Rect::new(
        canvas.x + (canvas.width - width) / 2,
        canvas.bottom().saturating_sub(height),
        width,
        height,
    );
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().bg(theme.surface).fg(theme.foreground),
        area,
    );
    let border = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(" TUI Draw · Chat ")
        .title_bottom(" Ctrl+G / Esc hide · PgUp/PgDn history ")
        .border_style(Style::default().fg(theme.focus));
    let inner = border.inner(area);
    frame.render_widget(border, area);
    let parts = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(inner);
    let mut lines = Vec::new();
    let history: Vec<_> = if project.session.chat.is_empty() {
        project
            .session
            .turns
            .iter()
            .take(project.session.cursor + 1)
            .map(|t| (t.prompt.as_str(), t.summary.as_str()))
            .collect()
    } else {
        project
            .session
            .chat
            .iter()
            .map(|t| (t.prompt.as_str(), t.reply.as_str()))
            .collect()
    };
    for (prompt, reply) in history
        .into_iter()
        .chain((!live.prompt.is_empty()).then_some((live.prompt.as_str(), live.reply.as_str())))
    {
        lines.push(Line::styled("You", Style::default().fg(theme.focus).bold()));
        lines.extend(chat_lines(prompt, parts[0].width, Style::default()));
        lines.push(Line::styled("TUI Draw", Style::default().bold()));
        lines.extend(chat_lines(reply, parts[0].width, Style::default()));
        lines.push(Line::raw(""));
    }
    for queued in &live.queued {
        lines.push(Line::styled(
            "You · queued",
            Style::default().fg(theme.muted),
        ));
        lines.extend(chat_lines(queued, parts[0].width, Style::default()));
    }
    if lines.is_empty() {
        lines.push(Line::from("Describe an interface to begin.").bold());
        lines.extend(chat_lines(
            "Then give feedback here. Successful changes save automatically.",
            parts[0].width,
            Style::default().fg(theme.muted),
        ));
    }
    if !status.starts_with("Saved v") {
        lines.push(Line::raw(""));
        lines.extend(chat_lines(
            status,
            parts[0].width,
            Style::default().fg(theme.muted),
        ));
    }
    let max_scroll = lines.len().saturating_sub(usize::from(parts[0].height));
    let offset = usize::from(offset).min(max_scroll);
    let top = max_scroll.saturating_sub(offset);
    frame.render_widget(
        Paragraph::new(lines.into_iter().skip(top).collect::<Vec<_>>()),
        parts[0],
    );
    let label = if building {
        " Message · Enter queue · Ctrl+X cancel "
    } else if project.current().is_some() {
        " Message · Enter send · Tab preview "
    } else {
        " Describe an interface · Enter build "
    };
    let border = Block::default().borders(Borders::TOP).title(label);
    let field = border.inner(parts[1]);
    frame.render_widget(border, parts[1]);
    let (visible, column) = prompt_window(input.0, input.1, field.width);
    frame.render_widget(
        Paragraph::new(if input.0.is_empty() {
            "What would you like to build or change?"
        } else {
            visible
        })
        .bg(theme.background)
        .fg(if input.0.is_empty() {
            theme.muted
        } else {
            theme.foreground
        }),
        field,
    );
    if !field.is_empty() {
        frame.set_cursor_position((field.x + column, field.y));
    }
    offset as u16
}

fn next_focus(ids: &[String], current: Option<&str>, reverse: bool) -> Option<String> {
    // None marks the boundary; preview callers wrap to the next control.
    let position = current
        .and_then(|id| ids.iter().position(|item| item == id))
        .map_or(0, |i| i + 1);
    let count = ids.len() + 1;
    let next = if reverse {
        (position + count - 1) % count
    } else {
        (position + 1) % count
    };
    next.checked_sub(1).map(|i| ids[i].clone())
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    mut spec: Spec,
    mut project: Project,
    engine: &str,
    mut chat_open: bool,
) -> Result<()> {
    let mut status = if project.current().is_some() {
        "Session resumed. Changes save automatically. Describe what to change."
    } else {
        "Describe your first interface below, or open a saved project with Ctrl+O."
    }
    .to_owned();
    let mut prompt = String::new();
    let mut cursor = 0;
    let mut chat_scroll: u16 = 0;
    let mut last_prompt = project
        .session
        .turns
        .get(project.session.cursor)
        .map(|t| t.prompt.clone())
        .unwrap_or_default();
    let mut focus: Option<String> = None;
    let mut scroll: u16 = 0;
    let mut job: Option<Job> = None;
    let mut live = LiveChat::default();
    let mut queue_paused = false;
    let mut navigation: Option<Navigation> = None;
    let mut quit_without_saving = false;
    project.remember();
    loop {
        let mut done = false;
        if let Some(active) = &job {
            while let Ok(event) = active.events.try_recv() {
                match event["type"].as_str() {
                    Some("assistant_start") => {
                        if !live.reply.is_empty() {
                            live.reply.push('\n');
                        }
                    }
                    Some("assistant_delta") => live
                        .reply
                        .push_str(event["text"].as_str().unwrap_or_default()),
                    Some("tool_start") => {
                        status = format!("Working · {}", event["name"].as_str().unwrap_or("tool"))
                    }
                    Some("tool_end") => {
                        status = if event["ok"] == false {
                            event["message"]
                                .as_str()
                                .unwrap_or("Checking the proposed changes again…")
                                .into()
                        } else {
                            format!("Done · {}", event["name"].as_str().unwrap_or("tool"))
                        }
                    }
                    Some("status") => {
                        status = event["message"]
                            .as_str()
                            .unwrap_or("Building interface…")
                            .into();
                    }
                    Some("step") => {
                        status = format!(
                            "Building · {}",
                            event["step"]["description"]
                                .as_str()
                                .unwrap_or("updating preview")
                        )
                    }
                    Some("complete") => {
                        let reason = event["stopReason"].as_str().unwrap_or("unknown");
                        status = match reason {
                            "finish" => match project.commit(&last_prompt, &event) {
                                Ok(()) => {
                                    if event["changed"] != false {
                                        spec =
                                            project.current().cloned().unwrap_or_else(empty_spec);
                                    }
                                    live.prompt.clear();
                                    live.reply.clear();
                                    if event["changed"] == false {
                                        "Reply saved".into()
                                    } else {
                                        format!(
                                            "Saved v{} in {} ms · {}",
                                            project.session.cursor + 1,
                                            event["elapsedMs"],
                                            event["summary"]
                                                .as_str()
                                                .unwrap_or("Ready for feedback")
                                        )
                                    }
                                }
                                Err(error) => {
                                    format!("Not saved: {error}. Previous screen retained.")
                                }
                            },
                            "unavailable" => {
                                "Layout unavailable for this request. Try a simpler description."
                                    .into()
                            }
                            _ => format!(
                                "Incomplete ({reason}) · Last preview retained. Enter to retry."
                            ),
                        };
                        done = true;
                    }
                    Some("error") => {
                        status = format!(
                            "{} · Enter to retry",
                            event["message"].as_str().unwrap_or("Composition failed")
                        );
                        done = true;
                    }
                    _ => {}
                }
            }
        }
        if done {
            if !status.starts_with("Saved v") {
                chat_open = true;
            }
            job = None;
            queue_paused = !live.prompt.is_empty();
            if queue_paused && prompt.is_empty() && live.queued.is_empty() {
                prompt.clone_from(&last_prompt);
                cursor = prompt.len();
            }
            chat_scroll = 0;
        }
        if job.is_none()
            && !queue_paused
            && let Some(next_prompt) = live.queued.pop_front()
        {
            let mut request = project.request(
                &next_prompt,
                (spec.content_height() > 0).then_some(&spec),
                engine,
            );
            request["viewport"] = json!([
                terminal.size()?.width.clamp(1, 500),
                terminal.size()?.height.saturating_sub(1).clamp(1, 200)
            ]);
            match generate(request) {
                Ok(next) => {
                    last_prompt = next_prompt;
                    live.prompt.clone_from(&last_prompt);
                    live.reply.clear();
                    job = Some(next);
                    chat_scroll = 0;
                    status =
                        "Thinking… Your current version stays until validation succeeds.".into();
                }
                Err(error) => {
                    live.queued.push_front(next_prompt);
                    queue_paused = true;
                    status =
                        format!("Cannot start agent: {error} · Enter to retry queued messages");
                }
            }
        }
        if focus
            .as_ref()
            .is_some_and(|id| !spec.focusable().contains(id))
        {
            focus = None;
        }
        if navigation.is_none()
            && !chat_open
            && let Some(id) = spec.active_popup()
        {
            focus = Some(id);
        }
        let viewport_height = terminal.size()?.height.saturating_sub(1);
        scroll = scroll.min(
            spec.content_height_in(terminal.size()?.width)
                .saturating_sub(viewport_height),
        );
        let changed = project.preview_changed(&spec);
        terminal.draw(|frame| {
            let areas =
                Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
            if spec.content_height() == 0 {
                let area = navigation::centered(areas[0], 60, 5);
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from("Your canvas is ready.").bold(),
                        Line::raw(""),
                        Line::raw("Open chat with Ctrl+G and describe what you want to build."),
                    ])
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: false }),
                    area,
                );
            } else {
                spec.render(
                    frame,
                    areas[0],
                    if chat_open || navigation.is_some() {
                        None
                    } else {
                        focus.as_deref()
                    },
                    scroll,
                );
            }
            let saved = if job.is_some() {
                "Building…"
            } else if changed {
                "Preview changed · Ctrl+S save"
            } else {
                "Saved"
            };
            let footer = if areas[1].width < 70 {
                " TUI Draw · ^G Chat · ^P Projects".into()
            } else {
                format!(
                    " TUI Draw · {} · {}  |  Ctrl+G Chat  Ctrl+P Projects  Ctrl+C Quit",
                    project
                        .dir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .chars()
                        .take(20)
                        .collect::<String>(),
                    saved
                )
            };
            frame.render_widget(
                Paragraph::new(clean_text(&footer)).fg(Color::Gray),
                areas[1],
            );
            if chat_open {
                chat_scroll = render_chat(
                    frame,
                    &project,
                    (&prompt, cursor),
                    (&status, &live, job.is_some()),
                    chat_scroll,
                    areas[0],
                );
            }
            if let Some(page) = &navigation {
                let area = navigation::centered(areas[0], 78, 25);
                frame.render_widget(Clear, area);
                let areas = Layout::vertical([
                    Constraint::Min(0),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .split(area);
                page.render(frame, areas[0]);
                frame.render_widget(
                    Paragraph::new(clean_text(&status)).wrap(Wrap { trim: false }),
                    areas[1],
                );
                let border =
                    Block::bordered().title(" Project folder · Enter confirm · Esc cancel ");
                let inner = border.inner(areas[2]);
                frame.render_widget(border, areas[2]);
                let (visible, column) = prompt_window(&page.input, page.cursor, inner.width);
                frame.render_widget(Paragraph::new(visible), inner);
                if page.page != Page::Help && !inner.is_empty() {
                    frame.set_cursor_position((inner.x + column, inner.y));
                }
            }
        })?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let event = event::read()?;
        if let Event::Paste(text) = &event {
            if chat_open || navigation.is_some() {
                let text = text.replace(['\r', '\n'], " ");
                if let Some(page) = navigation.as_mut() {
                    if page.page != Page::Help {
                        insert_prompt(&mut page.input, &mut page.cursor, &text);
                    }
                } else {
                    insert_prompt(&mut prompt, &mut cursor, &text);
                }
            }
            continue;
        }
        let Event::Key(mut key) = event else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        let control = key.modifiers == KeyModifiers::CONTROL;
        if control && key.code == KeyCode::Char('c') {
            if quit_without_saving {
                return Ok(());
            }
            if project.preview_changed(&spec)
                && let Err(error) = project.save_preview(&spec)
            {
                status =
                    format!("Save failed. Ctrl+C again quits without saving; Esc stays. {error}");
                chat_open = true;
                quit_without_saving = true;
                continue;
            }
            return Ok(());
        }
        quit_without_saving = false;
        if control && key.code == KeyCode::Char('g') && navigation.is_none() {
            chat_open = !chat_open;
            if !chat_open && focus.is_none() {
                focus = spec.focusable().first().cloned();
            }
            continue;
        }
        if key.code == KeyCode::Esc {
            if navigation.take().is_some() {
                status = "Returned to your draft.".into();
                continue;
            }
            if chat_open {
                chat_open = false;
                if focus.is_none() {
                    focus = spec.focusable().first().cloned();
                }
            } else if job.is_none()
                && let Some(id) = spec.active_popup()
                && let Err(error) = spec.activate(&id)
            {
                status = error;
                chat_open = true;
            }
            continue;
        }
        if control && key.code == KeyCode::Char('x') && job.take().is_some() {
            queue_paused = true;
            live.reply.push_str("\nCancelled. Draft discarded.");
            if prompt.is_empty() && live.queued.is_empty() {
                prompt.clone_from(&last_prompt);
                cursor = prompt.len();
            }
            status =
                "Cancelled · Previous interface retained. Enter sends or resumes queued messages."
                    .into();
            chat_open = true;
            continue;
        }
        if job.is_some()
            && (!chat_open
                || (control && key.code != KeyCode::Char('u'))
                || matches!(key.code, KeyCode::Tab | KeyCode::BackTab))
        {
            continue;
        }
        if control && job.is_none() {
            match key.code {
                KeyCode::Char('p') => {
                    if let Err(error) = project.save_preview(&spec) {
                        status = format!("Not saved: {error}");
                        chat_open = true;
                        continue;
                    }
                    let Some(next) = navigation::choose_project(terminal, &project, true)? else {
                        return Ok(());
                    };
                    let switched = next.dir != project.dir
                        || serde_json::to_string(&next.session)?
                            != serde_json::to_string(&project.session)?;
                    live = LiveChat::default();
                    queue_paused = false;
                    project = next;
                    spec = project.current().cloned().unwrap_or_else(empty_spec);
                    if switched {
                        last_prompt = project
                            .session
                            .turns
                            .get(project.session.cursor)
                            .map(|t| t.prompt.clone())
                            .unwrap_or_default();
                        prompt.clear();
                        cursor = 0;
                        scroll = 0;
                        chat_scroll = 0;
                        focus = None;
                        chat_open = project.current().is_none();
                        status = "Changes save automatically. Describe what to change.".into();
                    }
                    navigation = None;
                    continue;
                }
                KeyCode::Char('n') | KeyCode::Char('o') => {
                    navigation = Some(Navigation::new(
                        if key.code == KeyCode::Char('n') {
                            Page::New
                        } else {
                            Page::Open
                        },
                        &project,
                        "",
                    ));
                    focus = None;
                    status = "Choose a project folder. Esc keeps your current draft.".into();
                    continue;
                }
                KeyCode::Char('s') => {
                    status = project
                        .save_preview(&spec)
                        .map(|_| format!("Saved · {}", project.dir.display()))
                        .unwrap_or_else(|e| format!("Not saved: {e}"));
                    if status.starts_with("Not saved:") {
                        chat_open = true;
                    }
                    continue;
                }
                KeyCode::Char('z') if navigation.is_none() => {
                    if project.preview_changed(&spec)
                        && let Some(saved) = project.current()
                    {
                        spec = saved.clone();
                        focus = None;
                        status = "Restored saved preview.".into();
                        continue;
                    }
                    match project.undo() {
                        Ok(()) => {
                            spec = project.current().unwrap().clone();
                            focus = None;
                            scroll = 0;
                            status = format!("Restored v{}", project.session.cursor + 1);
                        }
                        Err(error) => status = error.to_string(),
                    }
                    continue;
                }
                _ => {}
            }
        }
        if let Some(page) = navigation.as_mut() {
            if page.page == Page::Help {
                continue;
            }
            if key.code == KeyCode::Enter {
                match switch_project(&mut project, &spec, page) {
                    Ok(next) => {
                        live = LiveChat::default();
                        queue_paused = false;
                        project = next;
                        spec = project.current().cloned().unwrap_or_else(empty_spec);
                        last_prompt = project
                            .session
                            .turns
                            .get(project.session.cursor)
                            .map(|t| t.prompt.clone())
                            .unwrap_or_default();
                        prompt.clear();
                        cursor = 0;
                        scroll = 0;
                        focus = None;
                        navigation = None;
                        chat_open = project.current().is_none();
                        chat_scroll = 0;
                        status = format!(
                            "Opened {} · Changes save automatically",
                            project.dir.display()
                        );
                    }
                    Err(error) => status = error.to_string(),
                }
            } else if key.code == KeyCode::Char('u') && control {
                page.input.clear();
                page.cursor = 0;
            } else if !control && !key.modifiers.contains(KeyModifiers::ALT) {
                page.edit(key.code);
            }
            continue;
        }
        if !chat_open && let Some(id) = spec.hotkey_target(key) {
            focus = Some(id.clone());
            scroll = spec
                .focus_offset_in(
                    &id,
                    Rect::new(0, 0, terminal.size()?.width, viewport_height),
                )
                .saturating_sub(viewport_height.saturating_sub(3));
            key.code = KeyCode::Enter;
            key.modifiers = KeyModifiers::NONE;
        } else if key.modifiers.contains(KeyModifiers::ALT) || (!chat_open && control) {
            continue;
        }
        if !chat_open && let Some(id) = focus.as_ref() {
            match spec.control_key(id, key.code) {
                Ok(true) => continue,
                Err(error) => {
                    status = error;
                    chat_open = true;
                    continue;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                if chat_open {
                    chat_open = false;
                    focus = None;
                }
                focus = next_focus(
                    &spec.focusable(),
                    focus.as_deref(),
                    key.code == KeyCode::BackTab,
                );
                if focus.is_none() {
                    focus = next_focus(&spec.focusable(), None, key.code == KeyCode::BackTab);
                }
                if let Some(id) = &focus {
                    scroll = spec
                        .focus_offset_in(
                            id,
                            Rect::new(0, 0, terminal.size()?.width, viewport_height),
                        )
                        .saturating_sub(viewport_height.saturating_sub(3));
                }
                continue;
            }
            KeyCode::PageDown => {
                if chat_open {
                    chat_scroll = chat_scroll.saturating_sub(5);
                } else {
                    scroll = scroll.saturating_add(viewport_height.max(1));
                }
                continue;
            }
            KeyCode::PageUp => {
                if chat_open {
                    chat_scroll = chat_scroll.saturating_add(5);
                } else {
                    scroll = scroll.saturating_sub(viewport_height.max(1));
                }
                continue;
            }
            _ => {}
        }
        if chat_open {
            match key.code {
                KeyCode::Char('u') if control => {
                    prompt.clear();
                    cursor = 0;
                }
                KeyCode::Up => {
                    prompt.clone_from(&last_prompt);
                    cursor = prompt.len();
                }
                KeyCode::Enter if prompt.trim().is_empty() && !live.queued.is_empty() => {
                    queue_paused = false;
                }
                KeyCode::Enter if !prompt.trim().is_empty() => {
                    if prompt.trim().starts_with('/') && job.is_some() {
                        status = "Cancel the active request before using project commands.".into();
                        continue;
                    }
                    if prompt.trim().starts_with('/') {
                        let (command, argument) = prompt
                            .trim()
                            .split_once(char::is_whitespace)
                            .unwrap_or((prompt.trim(), ""));
                        match command {
                            "/new" | "/open" | "/help" => {
                                let page=match command {"/new"=>Page::New,"/open"=>Page::Open,_=>Page::Help};
                                let next=Navigation::new(page,&project,argument.trim());
                                if !argument.trim().is_empty() && page!=Page::Help {
                                    match switch_project(&mut project,&spec,&next) {
                                        Ok(opened)=>{live=LiveChat::default();queue_paused=false;project=opened;spec=project.current().cloned().unwrap_or_else(empty_spec);prompt.clear();cursor=0;scroll=0;last_prompt=project.session.turns.get(project.session.cursor).map(|t|t.prompt.clone()).unwrap_or_default();status=format!("Opened {}",project.dir.display());},
                                        Err(error)=>status=error.to_string(),
                                    }
                                } else { navigation=Some(next);prompt.clear();cursor=0; }
                            }
                            "/save" | "/export" | "/undo" if argument.is_empty() => {
                                if command=="/undo" && project.preview_changed(&spec) && let Some(saved)=project.current() {
                                    spec=saved.clone();status="Restored saved preview.".into();prompt.clear();cursor=0;continue;
                                }
                                let result=if command=="/undo"{project.undo()}else{project.save_preview(&spec)};
                                match result {
                                    Ok(())=>{spec=project.current().cloned().unwrap_or_else(empty_spec);status=format!("Saved · {}",project.dir.display());prompt.clear();cursor=0;scroll=0;},
                                    Err(error)=>status=error.to_string(),
                                }
                            }
                            _=>status="Commands: /new [folder], /open [folder], /save, /undo, /export, /help".into(),
                        }
                        continue;
                    }
                    if live.queued.len() >= 16 {
                        status = "Queue is full (16 messages). Wait for a reply.".into();
                        continue;
                    }
                    live.queued.push_back(std::mem::take(&mut prompt));
                    cursor = 0;
                    queue_paused = false;
                    chat_scroll = 0;
                }
                code if !control => edit_prompt(&mut prompt, &mut cursor, code),
                _ => {}
            }
            continue;
        }
        let Some(id) = focus.as_ref() else { continue };
        if spec.elements[id].kind == "Keypad" {
            let result = match key.code {
                KeyCode::Left => spec.keypad_move(id, -1, 0),
                KeyCode::Right => spec.keypad_move(id, 1, 0),
                KeyCode::Up => spec.keypad_move(id, 0, -1),
                KeyCode::Down => spec.keypad_move(id, 0, 1),
                KeyCode::Enter | KeyCode::Char(' ') => spec.activate(id),
                KeyCode::Backspace => spec.keypad_type(id, "Backspace"),
                KeyCode::Delete => spec.keypad_type(id, "AC"),
                KeyCode::Char(c) if !control => spec.keypad_type(id, &c.to_string()),
                _ => continue,
            };
            if let Err(error) = result {
                status = error;
            }
            continue;
        }
        if spec.elements[id].kind == "Input" {
            match key.code {
                KeyCode::Char(c) if !control => {
                    if let Err(e) = spec.edit_input(id, Some(c)) {
                        status = e;
                    }
                    continue;
                }
                KeyCode::Backspace => {
                    if let Err(e) = spec.edit_input(id, None) {
                        status = e;
                    }
                    continue;
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                status = spec
                    .activate(id)
                    .map(|_| "Local action applied · Ctrl+G to open chat.".into())
                    .unwrap_or_else(|e| e)
            }
            KeyCode::Down => scroll = scroll.saturating_add(1),
            KeyCode::Up => scroll = scroll.saturating_sub(1),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_navigation_saves_preview_and_protects_existing_folders() -> Result<()> {
        let base = std::env::temp_dir().join(format!(
            "tui-navigation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let mut project = Project::create(base.join("first"))?;
        assert!(Project::load(&project.dir)?.current().is_none());
        let mut spec = Spec::parse(include_str!("../examples/primitives.json"))?;
        project.save_preview(&spec)?;
        project.save_preview(&spec)?;
        assert_eq!(project.session.turns.len(), 1);
        spec.state["count"] = json!("9");
        let nav = Navigation::new(Page::New, &project, "second project");
        let next = switch_project(&mut project, &spec, &nav)?;
        assert!(next.current().is_none());
        assert_eq!(project.current().unwrap().state["count"], "9");
        assert_eq!(project.session.turns.len(), 2);
        assert!(Project::create(&project.dir).is_err());
        assert!(Project::load(base.join("missing")).is_err());
        let mut unchanged = Project::load(&project.dir)?;
        std::fs::write(unchanged.dir.join("src/ui.rs"), "// User's source\n")?;
        spec.state["count"] = json!("8");
        let nav = Navigation::new(Page::New, &unchanged, "must-not-switch");
        assert!(switch_project(&mut unchanged, &spec, &nav).is_err());
        assert!(!base.join("must-not-switch").exists());
        assert_eq!(unchanged.current().unwrap().state["count"], "9");
        for page in [Page::New, Page::Open, Page::Help] {
            let view = Navigation::new(page, &project, "");
            for (width, height) in [(100, 25), (25, 8), (1, 1)] {
                let mut terminal =
                    Terminal::new(ratatui::backend::TestBackend::new(width, height))?;
                terminal.draw(|f| view.render(f, f.area()))?;
            }
        }
        std::fs::remove_dir_all(base)?;
        Ok(())
    }

    #[test]
    fn launcher_and_chat_render_without_resizing_the_canvas() -> Result<()> {
        let project = Project::open(std::env::temp_dir().join("tui-draw-render-only"))?;
        for (width, height) in [(100, 36), (32, 10), (1, 1)] {
            let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(width, height))?;
            terminal.draw(|frame| navigation::render_home(frame, &[], 0, "", false))?;
            if width == 100 {
                let text = buffer_text(terminal.backend().buffer());
                assert!(
                    text.contains("TUI Draw · Projects")
                        && text.contains("New project")
                        && text.contains("Delete")
                );
                assert!(text.lines().take(3).all(|line| line.trim().is_empty()));
            }
            terminal.draw(|frame| {
                frame.render_widget(Paragraph::new("CANVAS TOP"), frame.area());
                render_chat(
                    frame,
                    &project,
                    ("draft", 5),
                    ("Ready", &LiveChat::default(), false),
                    0,
                    frame.area(),
                );
            })?;
            if width == 100 {
                let text = buffer_text(terminal.backend().buffer());
                assert!(text.starts_with("CANVAS TOP"));
                assert!(text.contains("draft") && text.contains("TUI Draw · Chat"));
                assert!(!text.lines().next().unwrap().contains("Chat"));
            }
        }
        Ok(())
    }

    #[test]
    fn prompt_editing_and_focus_cycle() {
        let (mut text, mut cursor) = (String::new(), 0);
        insert_prompt(&mut text, &mut cursor, "A界é");
        edit_prompt(&mut text, &mut cursor, KeyCode::Left);
        edit_prompt(&mut text, &mut cursor, KeyCode::Backspace);
        edit_prompt(&mut text, &mut cursor, KeyCode::Char('!'));
        assert_eq!(text, "A!é");
        edit_prompt(&mut text, &mut cursor, KeyCode::Delete);
        assert_eq!(text, "A!");
        edit_prompt(&mut text, &mut cursor, KeyCode::Home);
        edit_prompt(&mut text, &mut cursor, KeyCode::Right);
        assert_eq!(cursor, 1);
        edit_prompt(&mut text, &mut cursor, KeyCode::End);
        assert_eq!(cursor, text.len());
        assert_eq!(prompt_window("A界é", "A界é".len(), 4), ("界é", 3));
        assert_eq!(prompt_window("A界é", "A界é".len(), 1), ("", 0));
        insert_prompt(&mut text, &mut cursor, &"界".repeat(4000));
        assert!(text.len() <= 4000 && text.is_char_boundary(cursor));
        let ids = vec!["input".into(), "button".into()];
        assert_eq!(next_focus(&ids, None, false).as_deref(), Some("input"));
        assert_eq!(
            next_focus(&ids, Some("input"), false).as_deref(),
            Some("button")
        );
        assert_eq!(next_focus(&ids, Some("button"), false), None);
        assert_eq!(next_focus(&ids, None, true).as_deref(), Some("button"));
        assert_eq!(next_focus(&ids, Some("input"), true), None);
        assert_eq!(next_focus(&[], None, false), None);
    }
}
