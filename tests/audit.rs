use ratatui_json::Spec;
use serde_json::json;
#[test]
fn audits_use_native_bounds_and_allow_scrolling_and_explicit_overrides() {
    let mut raw = json!({"root":"root","state":{"message":""},"elements":{
        "root":{"type":"Panel","props":{"title":"Chat","border":"double","padding":0,"gap":0},"children":["input","button","art"]},
        "input":{"type":"Input","props":{"label":"Message","value":{"$bindState":"/message"}}},
        "button":{"type":"Button","props":{"label":"Send","width":4},"on":{"press":{"action":"setState","params":{"statePath":"/message","value":"Sent"}}}},
        "art":{"type":"AnsiArt","props":{"title":"","content":"12345678901234567890","height":4}}
    }});
    raw["theme"] = serde_json::to_value(ratatui_json::theme::Theme::default()).unwrap();
    raw["theme"]["design"] = json!({"border":"rounded","density":"comfortable","emphasis":"quiet"});
    let spec = Spec::parse(&raw.to_string()).unwrap();
    let report = spec.layout_report(16, 5);
    assert!(report["contentHeight"].as_u64().unwrap() > 5);
    assert_eq!(report["nodes"]["root"]["inner"][0], 1); // explicit padding:0 wins
    let audits = report["audits"].as_array().unwrap();
    assert!(
        audits
            .iter()
            .any(|a| a["id"] == "button" && a["code"] == "control-fit")
    );
    assert!(
        audits
            .iter()
            .any(|a| a["id"] == "art" && a["severity"] == "warning")
    );
    raw["elements"]["button"]["props"]["width"] = json!("content");
    assert!(
        Spec::parse(&raw.to_string()).unwrap().layout_report(80, 24)["audits"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn shared_chrome_changes_rendering_and_measurement_and_survives_serialization() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut raw = json!({"root":"root","elements":{
        "root":{"type":"Panel","props":{"title":"Shared","padding":null,"border":null,"gap":null},"children":["input","metric"]},
        "input":{"type":"Input","props":{"label":"Name","value":{"$bindState":"/name"}}},
        "metric":{"type":"Metric","props":{"label":"Score","value":"42"}}
    },"state":{"name":"Ada"},"theme":ratatui_json::theme::Theme::default()});
    raw["theme"]["design"] = json!({"border":"double","density":"comfortable","emphasis":"quiet"});
    let spec = Spec::parse(&raw.to_string()).unwrap();
    let report = spec.layout_report(80, 24);
    assert_eq!(report["nodes"]["input"]["bounds"][0], 2);
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| spec.render(f, f.area(), None, 0))
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(0, 0)].symbol(), "╔");
    assert_eq!(terminal.backend().buffer()[(2, 2)].symbol(), "╔");
    raw["elements"]["root"]["props"]["surface"] = json!(true);
    let surfaced = Spec::parse(&raw.to_string()).unwrap();
    terminal
        .draw(|f| surfaced.render(f, f.area(), None, 0))
        .unwrap();
    assert_eq!(
        terminal.backend().buffer()[(1, 1)].bg,
        surfaced.theme.surface
    );
    assert_eq!(
        terminal.backend().buffer()[(3, 3)].bg,
        surfaced.theme.surface
    );
    assert_eq!(surfaced.layout_report(80, 24)["nodes"], report["nodes"]);
    let restored = Spec::parse(&serde_json::to_string(&spec).unwrap()).unwrap();
    assert_eq!(restored.layout_report(80, 24), report);
    raw["theme"]["design"]["density"] = json!("compact");
    assert_eq!(
        Spec::parse(&raw.to_string()).unwrap().layout_report(80, 24)["nodes"]["input"]["bounds"][0],
        1
    );
    raw["theme"]["design"]["border"] = json!("unknown");
    assert!(Spec::parse(&raw.to_string()).is_err());
}

#[test]
fn compact_controls_preserve_focus_actions_and_native_geometry() {
    use crossterm::event::KeyCode;
    use ratatui::{Terminal, backend::TestBackend, prelude::Rect};
    let mut raw = json!({"root":"root","state":{"choice":"Alpha","status":"Ready"},"elements":{
        "root":{"type":"Panel","props":{"title":"Choose","surface":true,"padding":1,"gap":1},"children":["choices","send","status"]},
        "choices":{"type":"Select","props":{"title":"Hidden title","bordered":false,"options":["Alpha","Beta"],"value":{"$bindState":"/choice"}}},
        "send":{"type":"Button","props":{"label":"Choose","variant":"plain","width":"content"},"on":{"press":{"action":"setState","params":{"statePath":"/status","value":{"$state":"/choice"}}}}},
        "status":{"type":"Text","props":{"text":{"$state":"/status"}}}
    }});
    let mut spec = Spec::parse(&raw.to_string()).unwrap();
    let layout = spec.layout_report(40, 20);
    assert!(layout["audits"].as_array().unwrap().is_empty());
    assert_eq!(layout["nodes"]["send"]["bounds"][3], 1);
    assert_eq!(layout["nodes"]["choices"]["bounds"][3], 2);
    spec.control_key("choices", KeyCode::Down).unwrap();
    spec.activate("send").unwrap();
    assert_eq!(spec.state["status"], "Beta");
    let mut terminal = Terminal::new(TestBackend::new(40, 20)).unwrap();
    terminal
        .draw(|f| spec.render(f, Rect::new(0, 0, 40, 20), Some("send"), 0))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let text = buffer
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("[ Choose ]") && !text.contains("Hidden title"));
    let bounds = &layout["nodes"]["send"]["bounds"];
    let cell = &buffer[(
        bounds[0].as_u64().unwrap() as u16,
        bounds[1].as_u64().unwrap() as u16,
    )];
    assert_eq!(cell.bg, spec.theme.focus);
    raw["elements"]["choices"]["props"]
        .as_object_mut()
        .unwrap()
        .remove("bordered");
    raw["elements"]["send"]["props"]
        .as_object_mut()
        .unwrap()
        .remove("variant");
    let legacy = Spec::parse(&raw.to_string()).unwrap().layout_report(40, 20);
    assert_eq!(legacy["nodes"]["send"]["bounds"][3], 3);
    assert_eq!(legacy["nodes"]["choices"]["bounds"][3], 4);
    // The same borderless contract applies to searchable and multi-select controls.
    for kind in ["Select", "MultiSelect"] {
        raw["elements"]["choices"]["type"] = json!(kind);
        raw["elements"]["choices"]["props"]["searchable"] = json!(true);
        raw["elements"]["choices"]["props"]["bordered"] = json!(false);
        raw["state"]["choice"] = if kind == "Select" {
            json!("Alpha")
        } else {
            json!([])
        };
        let mut spec = Spec::parse(&raw.to_string()).unwrap();
        spec.control_key("choices", KeyCode::Char('b')).unwrap();
        spec.control_key(
            "choices",
            if kind == "Select" {
                KeyCode::Enter
            } else {
                KeyCode::Char(' ')
            },
        )
        .unwrap();
        assert_eq!(
            spec.state["choice"],
            if kind == "Select" {
                json!("Beta")
            } else {
                json!(["Beta"])
            }
        );
        terminal
            .draw(|f| spec.render(f, Rect::new(0, 0, 40, 20), Some("choices"), 0))
            .unwrap();
        assert!(
            !terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
                .contains("Hidden title")
        );
    }
}

#[test]
fn hotkeys_measure_dispatch_and_respect_visibility_disabled_and_modal_scope() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    let mut raw = json!({"root":"root","state":{"result":"","open":false},"elements":{
        "root":{"type":"Column","props":{},"children":["button","hidden","disabled","popup"]},
        "button":{"type":"Button","props":{"label":"Run","hotkey":"Alt+R","variant":"plain","width":"content"},"on":{"press":{"action":"setState","params":{"statePath":"/result","value":"Ran"}}}},
        "hidden":{"type":"Button","props":{"label":"Hidden","hotkey":"F1"},"visible":false},
        "disabled":{"type":"Button","props":{"label":"Disabled","hotkey":"Alt+D","disabled":true}},
        "popup":{"type":"Popup","props":{"title":"Dialog","label":"Show","body":"Modal","open":{"$bindState":"/open"}}}
    }});
    let mut spec = Spec::parse(&raw.to_string()).unwrap();
    let alt_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT);
    assert_eq!(spec.hotkey_target(alt_r).as_deref(), Some("button"));
    assert!(
        spec.hotkey_target(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE))
            .is_none()
    );
    assert!(
        spec.hotkey_target(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL))
            .is_none()
    );
    assert!(
        spec.hotkey_target(KeyEvent::new_with_kind(
            KeyCode::Char('r'),
            KeyModifiers::ALT,
            KeyEventKind::Repeat
        ))
        .is_none()
    );
    assert!(
        spec.hotkey_target(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE))
            .is_none()
    );
    assert!(
        spec.hotkey_target(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT))
            .is_none()
    );
    let target = spec.hotkey_target(alt_r).unwrap();
    spec.activate(&target).unwrap();
    assert_eq!(spec.state["result"], "Ran");
    let bounds = &spec.layout_report(60, 20)["nodes"]["button"]["bounds"];
    assert_eq!(bounds[2], 15); // "Run · Alt+R" + four button cells.
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|f| spec.render(f, f.area(), Some("button"), 0))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("[ Run · Alt+R ]"));
    spec.activate("popup").unwrap();
    assert!(spec.hotkey_target(alt_r).is_none());
    raw["elements"]["hidden"]["props"]["hotkey"] = json!("Alt+r");
    assert!(
        Spec::parse(&raw.to_string())
            .unwrap_err()
            .contains("duplicate hotkey")
    );
    raw["elements"]["hidden"]["props"]["hotkey"] = json!("Ctrl+C");
    assert!(Spec::parse(&raw.to_string()).is_err());
}

#[test]
fn narrow_sidebar_reports_full_control_row_minimum_including_chrome() {
    let raw = json!({"root":"root","state":{"status":""},"elements":{
        "root":{"type":"Row","props":{},"children":["sidebar","status"]},
        "sidebar":{"type":"Panel","props":{"title":"Navigation","padding":1,"width":24},"children":["actions"]},
        "actions":{"type":"Row","props":{"gap":1},"children":["warp","scan"]},
        "warp":{"type":"Button","props":{"label":"Warp","hotkey":"Alt+W"},"on":{"press":{"action":"setState","params":{"statePath":"/status","value":"Warped"}}}},
        "scan":{"type":"Button","props":{"label":"Scan","hotkey":"Alt+S"},"on":{"press":{"action":"setState","params":{"statePath":"/status","value":"Scanned"}}}},
        "status":{"type":"Text","props":{"text":{"$state":"/status"}}}
    }});
    let report = Spec::parse(&raw.to_string())
        .unwrap()
        .layout_report(100, 35);
    assert_eq!(report["nodes"]["warp"]["minimumControlWidth"], 16);
    assert_eq!(report["nodes"]["actions"]["minimumControlWidth"], 33);
    assert_eq!(report["nodes"]["sidebar"]["minimumControlWidth"], 37);
    assert!(
        report["audits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == "warp" && a["requiredSize"] == json!([16, 3]))
    );
}
