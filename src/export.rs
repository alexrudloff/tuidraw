use crate::Result;
use ratatui_json::Spec;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

// Emit Rust literals, including Rust's Unicode escaping, rather than paste JSON into Rust strings.
fn literal(value: &Value) -> String {
    match value {
        Value::String(s) => format!("{s:?}"),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(literal).collect::<Vec<_>>().join(",")
        ),
        Value::Object(items) => format!(
            "{{{}}}",
            items
                .iter()
                .map(|(k, v)| format!("{k:?}:{}", literal(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => value.to_string(),
    }
}

pub fn source(spec: &Spec) -> String {
    let mut out = String::from(
        "// Generated layout. Edit through the builder; put application logic in actions.rs.\nuse generated_tui_runtime::{Spec, Element};\nuse serde_json::json;\nuse std::collections::BTreeMap;\n\npub fn build() -> Result<Spec, String> {\n    let elements = BTreeMap::from([\n",
    );
    for (id, e) in &spec.elements {
        out.push_str(&format!("        ({id:?}.into(), Element {{ kind: {:?}.into(), props: json!({}), children: vec![{}], on: serde_json::from_value(json!({})).map_err(|e| e.to_string())?, visible: {} }}),\n",
            e.kind, literal(&e.props), e.children.iter().map(|s|format!("{s:?}.into()")).collect::<Vec<_>>().join(","),
            literal(&serde_json::to_value(&e.on).unwrap()), e.visible.as_ref().map_or("None".into(), |v|format!("Some(json!({}))", literal(v)))));
    }
    out.push_str(&format!("    ]);\n    Spec::from_parts({:?}.into(), elements, json!({}), serde_json::from_value(json!({})).map_err(|e| e.to_string())?)\n}}\n",
        spec.root, literal(&spec.state), literal(&serde_json::to_value(spec.theme).unwrap())));
    out
}

pub fn files(spec: &Spec) -> BTreeMap<String, String> {
    let dependencies = include_str!("../Cargo.toml")
        .split_once("[dependencies]")
        .unwrap()
        .1;
    let mut files = BTreeMap::from([
        ("src/ui.rs".into(), source(spec)),
        (
            "ui.json".into(),
            serde_json::to_string_pretty(spec).unwrap() + "\n",
        ),
        (
            "runtime/Cargo.toml".into(),
            format!(
                "[package]\nname = \"generated-tui-runtime\"\nversion = \"0.1.0\"\nedition = \"2024\"\nlicense = \"MIT\"\n\n[dependencies]{dependencies}"
            ),
        ),
        (
            "THIRD_PARTY_NOTICES.md".into(),
            include_str!("../THIRD_PARTY_NOTICES.md").into(),
        ),
    ]);
    for (name, content) in [
        ("lib.rs", include_str!("lib.rs")),
        ("layout.rs", include_str!("layout.rs")),
        ("actions.rs", include_str!("actions.rs")),
        ("calculator.rs", include_str!("calculator.rs")),
        ("controls.rs", include_str!("controls.rs")),
        ("theme.rs", include_str!("theme.rs")),
        ("widgets.rs", include_str!("widgets.rs")),
        ("table.rs", include_str!("table.rs")),
        ("selection.rs", include_str!("selection.rs")),
    ] {
        files.insert(format!("runtime/src/{name}"), content.into());
    }
    files
}

pub fn scaffolding() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("Cargo.toml".into(), "[package]\nname = \"generated-terminal-app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nratatui = \"0.30\"\ncrossterm = \"0.29\"\nserde_json = \"1\"\ngenerated-tui-runtime = { path = \"runtime\" }\n".into()),
        ("src/main.rs".into(), include_str!("../templates/main.rs").into()),
        ("src/actions.rs".into(), "// Yours to edit. Regeneration and undo never overwrite this file.\nuse generated_tui_runtime::Spec;\n\n// Match stable widget IDs and return Ok(true) to replace their generated button action.\n// Return Ok(false) to run the generated local action.\npub fn handle(_spec: &mut Spec, _id: &str) -> Result<bool, String> {\n    Ok(false)\n}\n".into()),
        (".gitignore".into(), "/target/\n".into()),
        ("README.md".into(), "# Terminal application\n\nRun `cargo run`, validate with `cargo run -- --check`, or render without a terminal with `cargo run -- --snapshot`. No model service or Node is needed.\n\nTab / Shift+Tab selects controls; Enter activates; Ctrl+C quits.\n\n`src/ui.rs`, `ui.json`, and `runtime/` are generated. Edit the design through `tui-draw edit . \"feedback\"` or `tui-draw chat .`. `src/main.rs`, `src/actions.rs`, and this manifest are created once and yours to change. Use stable widget IDs in the action hook for application logic. Do not put credentials in UI state; it is saved in session history and exported source.\n\n`.builder/session.json` contains version history. `tui-draw undo .` restores the previous generated design without touching custom source. Generated files changed outside the builder are protected: an edit reports a conflict instead of overwriting them. Back up the changed file and restore its generated version before retrying.\n".into()),
    ])
}

// A change detector, not a security hash. Deterministic across processes and Rust versions.
pub fn fingerprint(text: &str) -> String {
    format!(
        "{:016x}",
        text.bytes()
            .fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b))
                .wrapping_mul(0x100000001b3))
    )
}

pub fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let parent = path.parent().ok_or("Missing parent directory")?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

pub fn write(
    dir: &Path,
    spec: &Spec,
    previous: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    spec.validate()?;
    let files = files(spec);
    // Preflight every managed file before writing anything. User source is create-once.
    for (name, content) in &files {
        let path = dir.join(name);
        if path.exists() {
            let current = fs::read_to_string(&path)?;
            if current != *content && previous.get(name) != Some(&fingerprint(&current)) {
                return Err(format!("Export conflict: {} was edited outside the builder; saved version and custom code retained", path.display()).into());
            }
        }
    }
    for (name, content) in scaffolding() {
        let path = dir.join(name);
        if !path.exists() {
            write_atomic(&path, &content)?;
        }
    }
    for (name, content) in &files {
        write_atomic(&dir.join(name), content)?;
    }
    Ok(files
        .iter()
        .map(|(name, content)| (name.clone(), fingerprint(content)))
        .collect())
}
