use crate::{Result, generate, interactive, project::Project};
use ratatui_json::Spec;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

pub const DEFAULT_ENGINE: &str = "jev";

pub const HELP: &str = "TUI Draw · conversational terminal app builder

  build \"interface description\" --out DIR [--engine llm|jev] [--json]
  build --spec FILE --out DIR [--json]       Import without a model call
  edit DIR \"feedback\" [--engine llm|jev] [--json]
  chat DIR [--engine llm|jev]                Create or resume a chat project
  inspect DIR [--json]                      Current spec, history and metrics
  layout DIR [--width N --height N] [--json] Measured widget bounds
  undo DIR [--json]                         Restore the previous version
  export DIR [--json]                       Regenerate Rust source locally
  --spec FILE --snapshot                    Render an existing spec

No arguments opens the project menu: create, open or delete a project.
Ctrl+P projects · Ctrl+G chat · Ctrl+S save · Ctrl+Z undo · /help
Esc hides chat. Enter queues feedback while busy; Ctrl+X cancels. Ctrl+C quits.
Default engine: jev. Use --engine llm for the single-model path.
Edits save source and a revision; questions save conversation only. Custom src/actions.rs,
src/main.rs and Cargo.toml are preserved. Model settings use the existing config.";

pub fn dispatch(args: &[String]) -> Result<()> {
    let command = args[0].as_str();
    let mut positional = vec![];
    let mut out = None;
    let mut spec_file = None;
    let mut engine = DEFAULT_ENGINE;
    let mut machine = false;
    let mut viewport = [100u16, 36u16];
    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => machine = true,
            "--out" => out = Some(iter.next().ok_or("--out requires DIR")?),
            "--spec" => spec_file = Some(iter.next().ok_or("--spec requires FILE")?),
            "--engine" => engine = iter.next().ok_or("--engine requires llm or jev")?,
            "--width" | "--height" => {
                let index = usize::from(arg == "--height");
                let value: u16 = iter.next().ok_or("Missing viewport dimension")?.parse()?;
                if value == 0 || value > [500, 200][index] {
                    return Err("Viewport width must be 1..500, height 1..200".into());
                }
                viewport[index] = value;
            }
            _ if arg.starts_with('-') => return Err(format!("Unknown option: {arg}").into()),
            _ => positional.push(arg.as_str()),
        }
    }
    if !["llm", "jev"].contains(&engine) {
        return Err("Engine must be llm or jev".into());
    }
    if command != "build" && (out.is_some() || spec_file.is_some()) {
        return Err("--out and --spec are build options".into());
    }
    let dir = if command == "build" {
        PathBuf::from(out.ok_or("build requires --out DIR")?)
    } else {
        PathBuf::from(positional.first().ok_or("Command requires a project DIR")?)
    };
    let expected = match command {
        "build" => usize::from(spec_file.is_none()),
        "edit" => 2,
        _ => 1,
    };
    if positional.len() != expected {
        return Err(format!("Wrong arguments for {command}; use --help").into());
    }
    let mut project = Project::open(dir)?;
    if command == "chat" {
        if machine {
            return Err("chat is interactive; use build/edit with --json for agents".into());
        }
        let spec = project.current().cloned().unwrap_or_else(crate::empty_spec);
        return interactive(spec, Some(project), engine);
    }
    let mut completion = None;
    match command {
        "build" | "edit" => {
            if command == "build" && project.current().is_some() {
                return Err("Project already exists; use edit or choose a new --out DIR".into());
            }
            if command == "edit" && project.current().is_none() {
                return Err("Project does not exist; use build first".into());
            }
            let prompt = if command == "edit" {
                positional[1]
            } else {
                positional.first().copied().unwrap_or("Imported interface")
            };
            let event = if let Some(file) = spec_file {
                let spec = Spec::parse(&fs::read_to_string(file)?)?;
                json!({"type":"complete","spec":spec,"summary":"Imported interface","engine":"import","elapsedMs":0})
            } else {
                let mut request = project.request(prompt, project.current(), engine);
                request["viewport"] = json!(viewport);
                let job = generate(request)?;
                loop {
                    let event = job
                        .events
                        .recv()
                        .map_err(|_| "Generation ended without a result")?;
                    match event["type"].as_str() {
                        Some("complete") => break event,
                        Some("error") => {
                            return Err(event["message"]
                                .as_str()
                                .unwrap_or("Generation failed")
                                .to_owned()
                                .into());
                        }
                        Some("status") => {
                            eprintln!("{}", event["message"].as_str().unwrap_or("Building…"))
                        }
                        _ => {}
                    }
                }
            };
            project.commit(prompt, &event)?;
            completion = Some(event);
        }
        "undo" => project.undo()?,
        "export" => project.export()?,
        "inspect" | "layout" => {
            if project.current().is_none() {
                return Err("Project has no interface".into());
            }
        }
        _ => return Err(format!("Unknown command: {command}").into()),
    }
    let mut report = project.report();
    if let Some(mut event) = completion {
        report["summary"] = event["summary"].clone();
        report["changed"] = event.get("changed").cloned().unwrap_or(json!(true));
        if let Some(fields) = event.as_object_mut() {
            for key in ["spec", "summary", "messages"] {
                fields.remove(key);
            }
        }
        report["metrics"] = event;
    }
    if command == "layout" {
        report["layout"] = project
            .current()
            .unwrap()
            .layout_report(viewport[0], viewport[1]);
        if !machine {
            println!("{}", serde_json::to_string_pretty(&report["layout"])?);
            return Ok(());
        }
    }
    if command == "inspect" {
        report["spec"] = serde_json::to_value(project.current())?;
        report["history"]=Value::Array(project.session.turns.iter().enumerate().map(|(i,t)|json!({"revision":i+1,"prompt":t.prompt,"summary":t.summary,"metrics":t.metrics})).collect());
    }
    if machine {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "{} · version {} · {}",
            project.dir.display(),
            report["revision"],
            report["summary"].as_str().unwrap_or("Ready")
        );
    }
    Ok(())
}
