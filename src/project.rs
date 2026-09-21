use crate::{Result, export};
use ratatui_json::Spec;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Turn {
    pub prompt: String,
    pub summary: String,
    pub spec: Spec,
    pub metrics: Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub prompt: String,
    pub reply: String,
    pub messages: Vec<Value>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    version: u32,
    pub turns: Vec<Turn>,
    #[serde(default)]
    pub chat: Vec<ChatTurn>,
    pub cursor: usize,
    managed: BTreeMap<String, String>,
}

pub struct Project {
    pub dir: PathBuf,
    pub session: Session,
    saved: Option<String>,
}

impl Project {
    pub fn create(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        if dir.exists() && fs::read_dir(dir)?.next().is_some() {
            return Err(
                "That folder is not empty. Open the existing project or choose a new folder."
                    .into(),
            );
        }
        let mut project = Self::open(dir)?;
        project.save(project.session.clone())?;
        Ok(project)
    }

    pub fn load(dir: impl AsRef<Path>) -> Result<Self> {
        if !dir.as_ref().join(".builder/session.json").is_file() {
            return Err("No saved project in that folder. Choose a project folder containing .builder/session.json.".into());
        }
        Self::open(dir)
    }

    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = std::path::absolute(dir)?;
        let dir = fs::canonicalize(&dir).unwrap_or(dir);
        let path = dir.join(".builder/session.json");
        let saved = match fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let session: Session = if let Some(text) = &saved {
            if text.len() > 64_000_000 {
                return Err("Session exceeds 64 MB".into());
            }
            serde_json::from_str(text)?
        } else {
            if dir.join("Cargo.toml").exists() {
                return Err("This directory contains a Rust project but no builder session; choose a new project directory".into());
            }
            Session {
                version: 1,
                turns: vec![],
                chat: vec![],
                cursor: 0,
                managed: BTreeMap::new(),
            }
        };
        if session.version != 1
            || (!session.turns.is_empty() && session.cursor >= session.turns.len())
        {
            return Err("Unsupported or invalid session".into());
        }
        for turn in &session.turns {
            turn.spec.validate()?;
        }
        Ok(Self {
            dir,
            session,
            saved,
        })
    }

    /// Deletion is a same-filesystem move so source and history remain recoverable.
    pub fn trash(dir: &Path) -> Result<PathBuf> {
        let project = Self::load(dir)?;
        if std::env::current_dir()?
            .canonicalize()?
            .starts_with(&project.dir)
        {
            return Err("Cannot delete the current working folder or its parent.".into());
        }
        let lock = project.dir.join(".builder/write.lock");
        let guard = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .map_err(|e| format!("Cannot delete a project while it is being written: {e}"))?;
        let result = (|| {
            let parent = project
                .dir
                .parent()
                .ok_or("Cannot delete the filesystem root")?;
            let trash = parent.join(".tui-draw-trash");
            fs::create_dir_all(&trash)?;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos();
            let target = trash.join(format!(
                "{}-{stamp}",
                project
                    .dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
            fs::rename(&project.dir, &target)?;
            Ok(target)
        })();
        drop(guard);
        let cleanup = match &result {
            Ok(target) => target.join(".builder/write.lock"),
            Err(_) => lock,
        };
        let _ = fs::remove_file(cleanup);
        result
    }

    /// Restore the most recent deletion without replacing a folder or a live writer.
    pub fn restore(trashed: &Path, original: &Path) -> Result<Self> {
        let project = Self::load(trashed)?;
        let lock = project.dir.join(".builder/write.lock");
        let guard = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .map_err(|e| format!("Cannot restore a project while it is being written: {e}"))?;
        let result = (|| {
            match fs::symlink_metadata(original) {
                Ok(_) => return Err("Cannot restore: a folder or file already exists at the original path. Move it first, then try Undo again.".into()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
                Err(e) => return Err(e.into()),
            }
            fs::rename(&project.dir, original)?;
            Ok(Self {
                dir: original.to_path_buf(),
                ..project
            })
        })();
        drop(guard);
        let cleanup = if result.is_ok() {
            original.join(".builder/write.lock")
        } else {
            lock
        };
        let _ = fs::remove_file(cleanup);
        result
    }

    pub fn current(&self) -> Option<&Spec> {
        self.session
            .turns
            .get(self.session.cursor)
            .map(|turn| &turn.spec)
    }

    pub fn preview_changed(&self, spec: &Spec) -> bool {
        // Preview controls only mutate state; generated structure is committed with its turn.
        self.current()
            .map_or(spec.content_height() > 0, |saved| saved.state != spec.state)
    }

    pub fn save_preview(&mut self, spec: &Spec) -> Result<()> {
        if self.preview_changed(spec) {
            let prompt = if self.current().is_some() {
                "Saved preview state"
            } else {
                "Imported interface"
            };
            self.commit(prompt,&json!({"spec":spec,"summary":"Saved preview state","engine":"local","elapsedMs":0}))
        } else {
            self.save(self.session.clone())
        }
    }

    fn recent_file() -> Option<PathBuf> {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
            .map(|base| base.join("ratatui-json/projects.json"))
    }

    pub fn recent_projects(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = Self::recent_file()
            .and_then(|p| fs::read_to_string(p).ok())
            .filter(|s| s.len() < 1_000_000)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        paths.insert(0, self.dir.clone());
        // Also discover older projects created before the recent-project index existed.
        if let Some(parent) = self.dir.parent()
            && let Ok(entries) = fs::read_dir(parent)
        {
            paths.extend(
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.join(".builder/session.json").is_file()),
            );
        }
        let mut recent = Vec::new();
        for path in paths {
            let path = fs::canonicalize(&path).unwrap_or(path);
            if path.join(".builder/session.json").is_file() && !recent.contains(&path) {
                recent.push(path);
            }
            if recent.len() == 32 {
                break;
            }
        }
        recent
    }

    pub fn remember(&self) {
        // CLI/PTY checks exercise the real index with an isolated XDG_STATE_HOME.
        if cfg!(test) {
            return;
        }
        if let Some(path) = Self::recent_file()
            && self.dir.join(".builder/session.json").is_file()
        {
            // Recents are a convenience; an unwritable index must not invalidate a saved project.
            let _ = export::write_atomic(
                &path,
                &serde_json::to_string(&self.recent_projects()).unwrap(),
            );
        }
    }

    pub fn request(&self, prompt: &str, initial: Option<&Spec>, engine: &str) -> Value {
        let history: Vec<_> = self
            .session
            .turns
            .iter()
            .take(self.session.cursor + 1)
            .rev()
            .take(6)
            .map(|t| json!({"prompt":t.prompt}))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let mut conversation: Vec<Value> =
            if self.session.chat.is_empty() {
                self.session.turns.iter().take(self.session.cursor + 1).map(|t| json!([
                {"role":"user","content":t.prompt},{"role":"assistant","content":t.summary}
            ])).collect()
            } else {
                self.session
                    .chat
                    .iter()
                    .map(|t| json!(t.messages))
                    .collect()
            };
        // Bound transport size before spawning Node; drop whole exchanges, never orphan tools.
        while !conversation.is_empty()
            && conversation
                .iter()
                .map(|g| g.to_string().len())
                .sum::<usize>()
                > 96_000
        {
            conversation.remove(0);
        }
        json!({"prompt":prompt,"initialSpec":initial,"engine":engine,"goal":self.session.turns.first().map(|t|&t.prompt),"history":history,"conversation":conversation,"diagnosticsPath":self.dir.join(".builder/last-failure.json")})
    }

    fn save(&mut self, next: Session) -> Result<()> {
        self.save_session(next, true)
    }

    fn save_session(&mut self, mut next: Session, export_source: bool) -> Result<()> {
        if serde_json::to_vec(&next)?.len() > 16_000_000 {
            return Err("Session history is full; import ui.json into a new project".into());
        }
        fs::create_dir_all(self.dir.join(".builder"))?;
        self.dir = fs::canonicalize(&self.dir)?;
        // A short local file lock prevents two CLI writers from interleaving exports.
        let lock_path = self.dir.join(".builder/write.lock");
        let _lock_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|e| {
                format!("Cannot lock project (another writer or stale .builder/write.lock): {e}")
            })?;
        struct Lock(PathBuf);
        impl Drop for Lock {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }
        let _lock = Lock(lock_path);
        let path = self.dir.join(".builder/session.json");
        let disk = match fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        if disk != self.saved {
            return Err("Project changed in another process; reopen it before editing".into());
        }
        if export_source && let Some(turn) = next.turns.get(next.cursor) {
            next.managed = export::write(&self.dir, &turn.spec, &self.session.managed)?;
        }
        let text = serde_json::to_string_pretty(&next)? + "\n";
        export::write_atomic(&path, &text)?;
        self.saved = Some(text);
        self.session = next;
        self.remember();
        Ok(())
    }

    pub fn commit(&mut self, prompt: &str, event: &Value) -> Result<()> {
        let mut next = self.session.clone();
        if let Some(messages) = event["messages"].as_array() {
            // Migrate old revision summaries once; keep the conversation separate from undo.
            if next.chat.is_empty() {
                next.chat = next
                    .turns
                    .iter()
                    .take(next.cursor + 1)
                    .map(|t| ChatTurn {
                        prompt: t.prompt.clone(),
                        reply: t.summary.clone(),
                        messages: vec![
                            json!({"role":"user","content":t.prompt}),
                            json!({"role":"assistant","content":t.summary}),
                        ],
                    })
                    .collect();
            }
            next.chat.push(ChatTurn {
                prompt: prompt.into(),
                reply: event["summary"].as_str().unwrap_or_default().into(),
                messages: messages.clone(),
            });
            // ponytail: keep 128 complete exchanges; add archival history if longer chats matter.
            if next.chat.len() > 128 {
                next.chat.drain(..next.chat.len() - 128);
            }
        }
        if event["changed"] == false {
            return self.save_session(next, false);
        }
        let spec = Spec::parse(&event["spec"].to_string())?;
        next.turns.truncate(if next.turns.is_empty() {
            0
        } else {
            next.cursor + 1
        });
        // ponytail: full snapshots capped at 64 turns; use a journal if longer histories are needed.
        if next.turns.len() >= 64 {
            return Err(
                "Session has 64 versions; start a new project with build --spec ui.json --out DIR"
                    .into(),
            );
        }
        let mut metrics = event.clone();
        if let Some(map) = metrics.as_object_mut() {
            map.remove("spec");
            map.remove("summary");
            map.remove("messages");
        }
        next.turns.push(Turn {
            prompt: prompt.into(),
            summary: event["summary"]
                .as_str()
                .unwrap_or("Updated interface")
                .into(),
            spec,
            metrics,
        });
        next.cursor = next.turns.len() - 1;
        self.save(next)
    }

    pub fn undo(&mut self) -> Result<()> {
        if self.session.cursor == 0 {
            return Err("No earlier version to restore".into());
        }
        let mut next = self.session.clone();
        next.cursor -= 1;
        self.save(next)
    }

    pub fn export(&mut self) -> Result<()> {
        if self.current().is_none() {
            return Err("Build an interface before exporting Rust source.".into());
        }
        self.save(self.session.clone())
    }

    pub fn report(&self) -> Value {
        let current = self.session.turns.get(self.session.cursor);
        json!({"ok":true,"project":self.dir,"revision":current.map(|_|self.session.cursor+1),"versions":self.session.turns.len(),"summary":current.map(|t|&t.summary),"metrics":current.map(|t|&t.metrics),"files":{"source":self.dir.join("src/ui.rs"),"actions":self.dir.join("src/actions.rs"),"spec":self.dir.join("ui.json"),"session":self.dir.join(".builder/session.json")},"run":"cargo run --manifest-path Cargo.toml"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_versions_undo_branching_and_source_conflict_protection() -> Result<()> {
        let dir = std::env::temp_dir().join(format!(
            "tui-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let mut project = Project::open(&dir)?;
        let spec = Spec::parse(include_str!("../examples/primitives.json"))?;
        let original = serde_json::to_value(&spec)?;
        project.commit(
            "Build controls",
            &json!({"spec":spec,"summary":"Controls","elapsedMs":5}),
        )?;
        let mut stale = Project::open(&dir)?;
        let hook = dir.join("src/actions.rs");
        fs::write(
            &hook,
            "// Application logic must survive generation and undo\n",
        )?;
        let custom = fs::read_to_string(&hook)?;
        let mut second = spec.clone();
        second.state["count"] = json!("5");
        project.commit(
            "Start count at five",
            &json!({"spec":second,"summary":"Set count"}),
        )?;
        assert_eq!(project.session.cursor, 1);
        assert!(
            stale
                .export()
                .unwrap_err()
                .to_string()
                .contains("another process")
        );
        let mut reopened = Project::open(&dir)?;
        assert_eq!(reopened.current().unwrap().state["count"], "5");
        reopened.undo()?;
        assert_eq!(serde_json::to_value(reopened.current())?, original);
        assert_eq!(fs::read_to_string(&hook)?, custom);
        reopened.commit("Branch from first version", &json!({"spec":spec}))?;
        assert_eq!(reopened.session.turns.len(), 2);
        assert_eq!(
            reopened.session.turns[1].prompt,
            "Branch from first version"
        );
        let before = fs::read_to_string(dir.join(".builder/session.json"))?;
        let ui = dir.join("src/ui.rs");
        fs::write(&ui, "// User changed this generated source\n")?;
        assert!(
            reopened
                .commit("Should conflict", &json!({"spec":second}))
                .unwrap_err()
                .to_string()
                .contains("Export conflict")
        );
        assert_eq!(
            fs::read_to_string(dir.join(".builder/session.json"))?,
            before
        );
        assert_eq!(
            fs::read_to_string(&ui)?,
            "// User changed this generated source\n"
        );
        assert_eq!(fs::read_to_string(&hook)?, custom);
        let reply = json!({"changed":false,"summary":"Explanation only","messages":[{"role":"user","content":"Explain"},{"role":"assistant","content":"Explanation only"}]});
        let versions = reopened.session.turns.len();
        reopened.commit("Explain", &reply)?;
        assert_eq!(reopened.session.turns.len(), versions);
        assert_eq!(
            fs::read_to_string(&ui)?,
            "// User changed this generated source\n"
        );
        let restored = Project::load(&dir)?;
        assert_eq!(
            restored.session.chat.last().unwrap().reply,
            "Explanation only"
        );
        assert!(
            !restored.request("Next", restored.current(), "llm")["conversation"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            stale
                .commit("Stale explanation", &reply)
                .unwrap_err()
                .to_string()
                .contains("another process")
        );
        let mut invalid = original;
        invalid["root"] = json!("missing");
        assert!(
            reopened
                .commit("Invalid", &json!({"spec":invalid}))
                .is_err()
        );
        assert!(!dir.join(".builder/write.lock").exists());
        fs::remove_dir_all(dir)?;
        Ok(())
    }
}
