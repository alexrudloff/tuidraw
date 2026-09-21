use ratatui::{Terminal, backend::TestBackend};
use ratatui_json::Spec;
use serde_json::json;

#[test]
fn palettes_color_the_surface_controls_and_focus_and_round_trip() {
    use ratatui::style::Color;
    let mut raw: serde_json::Value =
        serde_json::from_str(include_str!("../examples/primitives.json")).unwrap();
    raw["theme"] = serde_json::to_value(ratatui_json::theme::Theme::default()).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(100, 25)).unwrap();
    for (background, foreground, primary) in [
        ("#f3eee3", "#29251d", "#40683e"),
        ("#131025", "#e9e4f7", "#ba86ed"),
    ] {
        raw["theme"]["background"] = json!(background);
        raw["theme"]["foreground"] = json!(foreground);
        raw["theme"]["primary"] = json!(primary);
        let spec = Spec::parse(&raw.to_string()).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), None, 0))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(99, 24)].bg, spec.theme.background);
        assert_eq!(buffer[(0, 0)].fg, spec.theme.foreground);
        assert!(buffer.content.iter().any(|c| c.symbol() == "S"
            && c.bg == spec.theme.primary
            && c.fg == spec.theme.text_on(spec.theme.primary)));
        terminal
            .draw(|f| spec.render(f, f.area(), Some("sample"), 0))
            .unwrap();
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|c| c.symbol() == "S" && c.bg == spec.theme.focus)
        );
        let round_trip = Spec::parse(&serde_json::to_string(&spec).unwrap()).unwrap();
        assert_eq!(round_trip.theme.primary, spec.theme.primary);
        let mut same = spec.theme;
        same.foreground = same.background;
        assert!(matches!(
            same.text_on(same.background),
            Color::Black | Color::White
        ));
    }
    raw["theme"]["primary"] = json!("red");
    assert!(Spec::parse(&raw.to_string()).is_err());
}

#[test]
fn generic_grid_buttons_compute_and_commit_atomically() {
    let mut spec = Spec::parse(include_str!("../examples/primitives.json")).unwrap();
    assert_eq!(spec.focus_offset("sample"), spec.focus_offset("clear"));
    for (width, height) in [(100, 36), (28, 8), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), Some("sample"), 0))
            .unwrap();
    }
    for _ in 0..32 {
        spec.activate("sample").unwrap();
        let n: i32 = spec.state["result"].as_str().unwrap().parse().unwrap();
        assert!((5..=15).contains(&n));
    }
    spec.activate("increment").unwrap();
    assert_eq!(spec.state["count"], "3");
    spec.activate("clear").unwrap();
    assert_eq!(spec.state["result"], "Ready");
    spec.state["modifier"] = json!("not a number");
    let before = spec.state.clone();
    assert!(spec.activate("sample").is_err());
    assert_eq!(spec.state, before);
    spec.elements.get_mut("sample").unwrap().on.insert(
        "press".into(),
        json!([
            {"action":"setState","params":{"statePath":"/result","value":"Partial result"}},
            {"action":"setState","params":{"statePath":"/count","value":false}}
        ]),
    );
    assert!(spec.activate("sample").is_err());
    assert_eq!(spec.state, before);
    let mut source: serde_json::Value =
        serde_json::from_str(include_str!("../examples/primitives.json")).unwrap();
    source["elements"]["buttons"]["props"]["columns"] = json!(0);
    assert!(Spec::parse(&source.to_string()).is_err());
}

#[test]
fn replacing_calculator_with_dungeon_erases_the_entire_previous_preview() {
    let calculator = Spec::parse(include_str!("../examples/calculator.json")).unwrap();
    let dungeon = Spec::parse(include_str!("../examples/dungeon-scene.json")).unwrap();
    let mut reused = Terminal::new(TestBackend::new(100, 36)).unwrap();
    reused
        .draw(|f| calculator.render(f, f.area(), None, 0))
        .unwrap();
    reused
        .draw(|f| dungeon.render(f, f.area(), None, 0))
        .unwrap();
    let mut fresh = Terminal::new(TestBackend::new(100, 36)).unwrap();
    fresh
        .draw(|f| dungeon.render(f, f.area(), None, 0))
        .unwrap();
    assert_eq!(reused.backend().buffer(), fresh.backend().buffer());
}

#[test]
fn graphics_preserve_color_and_contain_terminal_sequences() {
    use ratatui::style::Color;
    let raw = json!({"root":"art","state":{},"elements":{"art":{"type":"AnsiArt","props":{"title":"Art","height":6,"content":"\u{1b}[31mR\u{1b}[38;5;123mI\u{1b}[38;2;12;34;56mT\u{1b}[0m\u{1b}]52;c;SECRET\u{7}\u{1b}[2J\nSHIP"}}}});
    let spec = Spec::parse(&raw.to_string()).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
    terminal
        .draw(|f| spec.render(f, f.area(), None, 0))
        .unwrap();
    let cells = &terminal.backend().buffer().content;
    for (symbol, color) in [
        ("R", Color::Red),
        ("I", Color::Indexed(123)),
        ("T", Color::Rgb(12, 34, 56)),
    ] {
        assert!(
            cells
                .iter()
                .any(|cell| cell.symbol() == symbol && cell.fg == color)
        );
    }
    let text: String = cells.iter().map(|c| c.symbol()).collect();
    assert!(text.contains("SHIP") && !text.contains("SECRET"));
    assert!(text.chars().all(|c| !c.is_control()));
    let mut escaped = raw.clone();
    escaped["elements"]["art"]["props"]["content"] = json!(r"\u001b[31mR\u001b[0m\nSHIP");
    let escaped = Spec::parse(&escaped.to_string()).unwrap();
    terminal
        .draw(|f| escaped.render(f, f.area(), None, 0))
        .unwrap();
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|c| c.symbol() == "R" && c.fg == Color::Red)
    );
    let gallery: serde_json::Value =
        serde_json::from_str(include_str!("../examples/widget-gallery.json")).unwrap();
    let mut scene =
        json!({"root":"scene","state":{},"elements":{"scene":gallery["elements"]["graphics1"]}});
    let spec = Spec::parse(&scene.to_string()).unwrap();
    terminal
        .draw(|f| spec.render(f, f.area(), None, 0))
        .unwrap();
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|c| c.symbol() == "@" && c.fg == Color::Rgb(255, 176, 0))
    );
    scene["elements"]["scene"]["props"]["rows"][0] = json!("bad\u{1b}");
    assert!(Spec::parse(&scene.to_string()).is_err());
}

#[test]
fn expanded_widgets_render_and_controls_update_locally() {
    use crossterm::event::KeyCode;
    let source = include_str!("../examples/widget-gallery.json");
    let mut spec = Spec::parse(source).unwrap();
    // Render every recipe separately so none can hide below the global viewport.
    for (id, element) in &spec.elements {
        if id == "gallery" {
            continue;
        }
        let single = json!({"root":id,"state":spec.state,"elements":{id:element}});
        let single = Spec::parse(&single.to_string()).unwrap();
        for (width, height) in [(100, 32), (28, 8), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|f| single.render(f, f.area(), Some(id), 0))
                .unwrap();
            if width == 100 {
                assert!(
                    terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .any(|c| c.symbol() != " "),
                    "Empty {}",
                    element.kind
                );
            }
        }
    }
    spec.control_key("widget8", KeyCode::Down).unwrap();
    assert_eq!(spec.state["widget8"], "fr");
    spec.control_key("widget9", KeyCode::Right).unwrap();
    assert_eq!(spec.state["widget9"], "B");
    spec.control_key("widget10", KeyCode::Right).unwrap();
    assert_eq!(spec.state["widget10"], 51.0);
    spec.control_key("widget10", KeyCode::End).unwrap();
    spec.control_key("widget10", KeyCode::Right).unwrap();
    assert_eq!(spec.state["widget10"], 100.0);
    spec.activate("widget7").unwrap();
    assert_eq!(spec.active_popup().as_deref(), Some("widget7"));
    assert_eq!(spec.focusable(), ["widget7"]);
    let mut terminal = Terminal::new(TestBackend::new(80, 25)).unwrap();
    terminal
        .draw(|f| spec.render(f, f.area(), Some("widget7"), 0))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("Enter or Esc to close"));
    spec.activate("widget7").unwrap();
    assert!(spec.active_popup().is_none());
    let single =
        json!({"root":"widget6","state":{},"elements":{"widget6":spec.elements["widget6"]}});
    let mut scroller = Spec::parse(&single.to_string()).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(80, 8)).unwrap();
    terminal
        .draw(|f| scroller.render(f, f.area(), Some("widget6"), 0))
        .unwrap();
    scroller.control_key("widget6", KeyCode::End).unwrap();
    terminal
        .draw(|f| scroller.render(f, f.area(), Some("widget6"), 0))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("Log entry 40"));
    let mut bad: serde_json::Value = serde_json::from_str(source).unwrap();
    bad["elements"]["widget4"]["props"]["values"] = json!([1]);
    assert!(Spec::parse(&bad.to_string()).is_err());
    bad = serde_json::from_str(source).unwrap();
    bad["state"]["widget8"] = json!("not an option");
    assert!(Spec::parse(&bad.to_string()).is_err());
    bad = serde_json::from_str(source).unwrap();
    bad["elements"]["widget10"]["props"]["min"] = json!(100);
    assert!(Spec::parse(&bad.to_string()).is_err());
}

#[test]
fn compound_controls_calculate_toggle_and_render_at_small_sizes() {
    let mut spec = Spec::parse(include_str!("../examples/calculator.json")).unwrap();
    assert_eq!(spec.focusable(), ["calculator"]);
    for key in ["7", "+", "8"] {
        spec.keypad_type("calculator", key).unwrap();
    }
    spec.activate("calculator").unwrap();
    assert_eq!(spec.state["display"], "15");
    spec.keypad_type("calculator", "4").unwrap();
    assert_eq!(
        spec.state["display"], "4",
        "digits after equals start a new calculation"
    );
    // Arrow navigation selects the actual AC button; pressing it clears locally.
    spec.keypad_move("calculator", 1, 1).unwrap();
    spec.activate("calculator").unwrap();
    assert_eq!(spec.state["display"], "0");
    for (width, height) in [(100, 32), (38, 23), (28, 8), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), Some("calculator"), 0))
            .unwrap();
        if width == 100 {
            let buffer = terminal.backend().buffer();
            let lines: Vec<String> = (0..height)
                .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
                .collect();
            assert!(lines.iter().any(|line| line.contains('7')
                && line.contains('8')
                && line.contains('9')
                && line.contains('×')));
            assert!(lines.iter().any(|line| line.contains('1')
                && line.contains('2')
                && line.contains('3')
                && line.contains('+')));
            assert!(lines.iter().any(|line| line.contains("Calculator")));
            assert!(
                buffer
                    .content
                    .iter()
                    .any(|cell| matches!(cell.bg, ratatui::style::Color::Rgb(..)))
            );
        }
    }
    let controls = json!({"root":"root","state":{"enabled":false,"number":"0"},"elements":{
        "root":{"type":"Column","props":{},"children":["toggle","badge","disabled","numeric"]},
        "toggle":{"type":"Switch","props":{"label":"Notifications","checked":{"$bindState":"/enabled"},"disabled":false}},
        "badge":{"type":"Badge","props":{"label":"Connected","intent":"success"}},
        "disabled":{"type":"Button","props":{"label":"Unavailable","intent":"danger","disabled":true},"on":{"press":{"action":"setState","params":{"statePath":"/enabled","value":true}}}},
        "numeric":{"type":"Keypad","props":{"title":"PIN","mode":"numeric","value":{"$bindState":"/number"}}}
    }});
    let mut controls = Spec::parse(&controls.to_string()).unwrap();
    assert_eq!(controls.focusable(), ["toggle", "numeric"]);
    controls.activate("toggle").unwrap();
    assert_eq!(controls.state["enabled"], true);
    assert!(controls.activate("disabled").is_err());
    for key in ["1", "2", "+", "3", "Backspace"] {
        controls.keypad_type("numeric", key).unwrap();
    }
    assert_eq!(controls.state["number"], "12");
    let mut terminal = Terminal::new(TestBackend::new(80, 32)).unwrap();
    terminal
        .draw(|f| controls.render(f, f.area(), Some("toggle"), 0))
        .unwrap();
    let mut invalid = serde_json::to_value(&controls).unwrap();
    invalid["state"]["enabled"] = json!("true");
    assert!(Spec::parse(&invalid.to_string()).is_err());
    invalid = serde_json::to_value(&spec).unwrap();
    invalid["state"]["display"] = json!("1".repeat(129));
    assert!(Spec::parse(&invalid.to_string()).is_err());
}

#[test]
fn spec_render_interact_and_reject_invalid_input() {
    let source = include_str!("../examples/demo.json");
    let mut spec = Spec::parse(source).unwrap();
    let focus = spec.focusable();
    assert_eq!(focus, ["node_6", "node_7"]);
    spec.edit_input(&focus[0], Some('é')).unwrap();
    assert_eq!(spec.state["name"], "Adaé");
    spec.edit_input(&focus[0], None).unwrap();
    spec.activate(&focus[1]).unwrap();
    assert_eq!(spec.state["status"], "Saved locally");
    for (width, height) in [(100, 40), (28, 8), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), Some(&focus[0]), 0))
            .unwrap();
        if width == 100 {
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            for expected in [
                "OPERATIONS OVERVIEW",
                "$48,290",
                "42%",
                "Database",
                "Saved locally",
            ] {
                assert!(text.contains(expected), "Missing {expected}");
            }
        }
    }
    let mut bad: serde_json::Value = serde_json::from_str(source).unwrap();
    bad["elements"]["node_0"]["children"] = json!(["node_0"]);
    assert!(Spec::parse(&bad.to_string()).is_err());
    let mut bad: serde_json::Value = serde_json::from_str(source).unwrap();
    bad["elements"]["node_7"]["on"]["press"]["action"] = json!("exec");
    assert!(Spec::parse(&bad.to_string()).is_err());
    let mut bad: serde_json::Value = serde_json::from_str(source).unwrap();
    bad["state"]["cpu"] = json!(101);
    assert!(Spec::parse(&bad.to_string()).is_err());
    let mut bad: serde_json::Value = serde_json::from_str(source).unwrap();
    bad["elements"]["node_7"]["on"]["press"]["params"] = json!({"statePath":"/cpu", "value":101});
    let mut bad = Spec::parse(&bad.to_string()).unwrap();
    assert!(bad.activate("node_7").is_err());
    assert_eq!(bad.state["cpu"], 42, "invalid state change must roll back");
    let mut hidden: serde_json::Value = serde_json::from_str(source).unwrap();
    hidden["elements"]["node_6"]["visible"] = json!({"$state":"/name", "eq":"Nobody"});
    let hidden = Spec::parse(&hidden.to_string()).unwrap();
    assert_eq!(hidden.focusable(), ["node_7"]);
    let roundtrip = serde_json::to_value(&spec).unwrap();
    assert!(roundtrip["elements"]["node_0"].get("visible").is_none());
}

/// A Row of three Columns lays its headings out side by side (same y, increasing x),
/// while the identical tree with a Column root stacks them (increasing y).
#[test]
fn row_of_three_columns_shares_y_then_column_stacks_headings() {
    fn tree(container: &str) -> serde_json::Value {
        json!({"root":"root","state":{},"elements":{
            "root":{"type":container,"props":{},"children":["c1","c2","c3"]},
            "c1":{"type":"Column","props":{},"children":["t1"]},
            "t1":{"type":"Text","props":{"text":"LEFT","muted":false}},
            "c2":{"type":"Column","props":{},"children":["t2"]},
            "t2":{"type":"Text","props":{"text":"CENTER","muted":false}},
            "c3":{"type":"Column","props":{},"children":["t3"]},
            "t3":{"type":"Text","props":{"text":"RIGHT","muted":false}}
        }})
    }
    const WIDTH: u16 = 120;
    const HEIGHT: u16 = 8;

    let positions = |container: &str| -> Vec<(u16, u16)> {
        let spec = Spec::parse(&tree(container).to_string()).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), None, 0))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        ["LEFT", "CENTER", "RIGHT"]
            .into_iter()
            .map(|heading| {
                (0..HEIGHT)
                    .find_map(|y| {
                        let line: String = (0..WIDTH).map(|x| buffer[(x, y)].symbol()).collect();
                        line.find(heading).map(|start| (y, start as u16))
                    })
                    .unwrap_or_else(|| panic!("{heading} missing under {container}"))
            })
            .collect()
    };

    let row = positions("Row");
    assert!(
        row.windows(2).all(|p| p[0].0 == p[1].0),
        "Row headings must share one line: {row:?}"
    );
    assert!(
        row.windows(2).all(|p| p[0].1 < p[1].1),
        "Row headings must advance in x: {row:?}"
    );

    let column = positions("Column");
    assert!(
        column.windows(2).all(|p| p[0].0 < p[1].0),
        "Column headings must advance in y: {column:?}"
    );
    assert!(
        column.iter().all(|p| p.1 == column[0].1),
        "Column headings must keep one x: {column:?}"
    );
}

#[test]
fn flexible_content_fills_viewport_and_keeps_composer_at_bottom() {
    use ratatui::layout::Rect;
    let tree = json!({"root":"root","state":{"draft":"hello"},"elements":{
        "root":{"type":"Column","props":{},"children":["heading","body","hidden"]},
        "heading":{"type":"Text","props":{"text":"HEADING"}},
        "body":{"type":"Row","props":{},"children":["panel"]},
        "panel":{"type":"Panel","props":{"title":"Main"},"children":["log","input"]},
        "log":{"type":"ScrollView","props":{"grow":1,"title":"Conversation","height":4,"text":"one\ntwo"}},
        "input":{"type":"Input","props":{"label":"Message","value":{"$bindState":"/draft"}}},
        "hidden":{"type":"Text","props":{"grow":16,"text":"HIDDEN"},"visible":false}
    }});
    let spec = Spec::parse(&tree.to_string()).unwrap();
    assert_eq!(spec.content_height(), 11);
    for height in [11, 20, 40, 70] {
        let mut terminal = Terminal::new(TestBackend::new(80, height)).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), Some("input"), 0))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let line = |y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>();
        assert!(line(3).contains("Conversation"));
        assert!(line(height - 4).contains("Message"));
        assert_eq!(buffer[(1, height - 2)].symbol(), "└");
        assert_eq!(buffer[(0, height - 1)].symbol(), "└");
        assert_eq!(
            spec.focus_offset_in("input", Rect::new(9, 5, 80, height)),
            height - 4
        );
        assert!(!(0..height).any(|y| line(y).contains("HIDDEN")));
    }
    // Explicit zero stops inherited growth; old fixed layouts remain compact.
    let mut fixed = tree.clone();
    fixed["elements"]["body"]["props"]["grow"] = json!(0);
    let fixed = Spec::parse(&fixed.to_string()).unwrap();
    assert_eq!(fixed.focus_offset_in("input", Rect::new(0, 0, 80, 40)), 7);
    for (width, height) in [(1, 1), (28, 8)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), Some("input"), 0))
            .unwrap();
    }
    for value in [json!(-1), json!(17), json!(1.5), json!("fill"), json!(null)] {
        let mut bad = tree.clone();
        bad["elements"]["log"]["props"]["grow"] = value;
        assert!(Spec::parse(&bad.to_string()).is_err());
    }
    // Grid rows grow too: weights 1:3 divide 12 extra rows as 3:9.
    let grid = json!({"root":"grid","elements":{
        "grid":{"type":"Grid","props":{"columns":1},"children":["a","b"]},
        "a":{"type":"Input","props":{"grow":1,"label":"A","value":{"$bindState":"/draft"}}},
        "b":{"type":"Input","props":{"grow":3,"label":"B","value":{"$bindState":"/draft"}}}
    },"state":{"draft":""}});
    let grid = Spec::parse(&grid.to_string()).unwrap();
    assert_eq!(grid.focus_offset_in("b", Rect::new(0, 0, 80, 18)), 6);
}

#[test]
fn adaptive_layout_and_local_selection_survive_resize_and_reload() {
    use crossterm::event::KeyCode;
    use ratatui::layout::Rect;
    let mut spec = Spec::parse(include_str!("../examples/adaptive-controls.json")).unwrap();
    assert_eq!(spec.focusable(), ["destination", "cargo", "clear"]);
    let original = spec.state.clone();
    for ch in "eps".chars() {
        spec.control_key("destination", KeyCode::Char(ch)).unwrap();
    }
    // Typing and highlighting do not commit a choice.
    assert_eq!(spec.state, original);
    spec.control_key("destination", KeyCode::Enter).unwrap();
    // ASCII 'eps' cannot match accented É. Unicode search itself is supported.
    assert_eq!(spec.state["destination"], "Sol");
    for _ in 0..3 {
        spec.control_key("destination", KeyCode::Backspace).unwrap();
    }
    for ch in "éps".chars() {
        spec.control_key("destination", KeyCode::Char(ch)).unwrap();
    }
    spec.control_key("destination", KeyCode::Enter).unwrap();
    assert_eq!(spec.state["destination"], "Épsilon Eridani");
    for ch in "mds".chars() {
        spec.control_key("cargo", KeyCode::Char(ch)).unwrap();
    }
    spec.control_key("cargo", KeyCode::Char(' ')).unwrap();
    assert_eq!(spec.state["cargo"], json!(["Fuel", "Medical supplies"]));
    // Query/cursor are transient; choices survive serialization.
    let restored = Spec::parse(&serde_json::to_string(&spec).unwrap()).unwrap();
    assert_eq!(restored.state, spec.state);
    spec.activate("clear").unwrap();
    assert_eq!(spec.state["cargo"], json!([]));
    for (width, height) in [(120, 48), (50, 60), (1, 1)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| restored.render(f, f.area(), Some("cargo"), 0))
            .unwrap();
        if width > 1 {
            let buffer = terminal.backend().buffer();
            let locate = |needle: &str| {
                (0..height)
                    .find_map(|y| {
                        let line: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
                        line.find(needle).map(|x| (x, y))
                    })
                    .unwrap_or_else(|| panic!("Missing {needle} at width {width}"))
            };
            let controls = locate("Flight plan");
            let table = locate("Market /");
            if width == 120 {
                assert_eq!(controls.1, table.1);
                assert!(table.0 > controls.0);
            } else {
                assert!(table.1 > controls.1);
            }
            assert_eq!(
                restored.focus_offset_in("clear", Rect::new(0, 0, width, height)),
                locate("Clear cargo").1 - 1
            );
        }
    }
    assert!(restored.content_height_in(50) > restored.content_height_in(120));
    let grid = json!({"root":"grid","state":{"value":""},"elements":{
        "grid":{"type":"Grid","props":{"columns":4,"gap":1,"minCellWidth":10},"children":["a","b","c","d"]},
        "a":{"type":"Input","props":{"label":"A","value":{"$bindState":"/value"}}},
        "b":{"type":"Input","props":{"label":"B","value":{"$bindState":"/value"}}},
        "c":{"type":"Input","props":{"label":"C","value":{"$bindState":"/value"}}},
        "d":{"type":"Input","props":{"label":"D","value":{"$bindState":"/value"}}}
    }});
    let grid = Spec::parse(&grid.to_string()).unwrap();
    assert_eq!(grid.content_height_in(80), 3);
    assert_eq!(grid.content_height_in(25), 7);
    assert_eq!(grid.focus_offset_in("d", Rect::new(0, 0, 25, 7)), 4);
    let mut terminal = Terminal::new(TestBackend::new(25, 7)).unwrap();
    terminal
        .draw(|f| grid.render(f, f.area(), Some("d"), 0))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let row: String = (0..25).map(|x| buffer[(x, 4)].symbol()).collect();
    assert!(row.contains('C') && row.contains('D'));

    let mut bad = serde_json::to_value(&restored).unwrap();
    bad["state"]["cargo"] = json!(["unknown"]);
    assert!(Spec::parse(&bad.to_string()).is_err());
}

#[test]
fn row_width_caps_release_space_and_measure_nested_layout_at_its_real_width() {
    use ratatui::layout::Rect;
    let raw = json!({"root":"root","state":{"draft":""},"elements":{
        "root":{"type":"Column","props":{},"children":["body","composer"]},
        "body":{"type":"Row","props":{"gap":1,"collapseBelow":60},"children":["rail","main"]},
        "rail":{"type":"Panel","props":{"title":"Rail","maxWidth":24},"children":["label"]},
        "label":{"type":"Text","props":{"text":"Navigation"}},
        "main":{"type":"Row","props":{"gap":1,"collapseBelow":70},"children":["a","b"]},
        "a":{"type":"Input","props":{"label":"A","value":{"$bindState":"/draft"}}},
        "b":{"type":"Input","props":{"label":"B","value":{"$bindState":"/draft"}}},
        "composer":{"type":"Row","props":{"gap":1,"collapseBelow":60},"children":["input","send"]},
        "input":{"type":"Input","props":{"label":"Message","value":{"$bindState":"/draft"}}},
        "send":{"type":"Button","props":{"label":"Send","maxWidth":12,"horizontalAlign":"right"},"on":{"press":{"action":"setState","params":{"statePath":"/draft","value":""}}}}
    }});
    let spec = Spec::parse(&raw.to_string()).unwrap();
    assert!(spec.content_height_in(80) > spec.content_height_in(100));
    for width in [100, 80, 40, 1] {
        let height = spec.content_height_in(width);
        let viewport = Rect::new(0, 0, width, height);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), None, 0))
            .unwrap();
        let buffer = terminal.backend().buffer();
        if width >= 80 {
            // The rail owns 24 columns, not half the viewport. Its sibling starts after one gap.
            assert_eq!(buffer[(23, 0)].symbol(), "┐");
            assert_eq!(buffer[(25, 0)].symbol(), "┌");
            let y = spec.focus_offset_in("input", viewport);
            assert_eq!(buffer[(width - 14, y)].symbol(), "┐");
            assert_eq!(buffer[(width - 13, y)].symbol(), " ");
            assert_eq!(buffer[(width - 12, y)].symbol(), "╭");
            assert_eq!(buffer[(width - 1, y)].symbol(), "╮");
            let b_y = spec.focus_offset_in("b", viewport);
            assert_eq!(b_y, if width == 100 { 0 } else { 4 });
            assert_eq!(buffer[(25, b_y)].symbol(), "┌");
        } else if width == 40 {
            // Collapsed rows keep full-width slots and do not cap the stacked main region.
            let b_y = spec.focus_offset_in("b", viewport);
            assert_eq!(buffer[(0, b_y)].symbol(), "┌");
            assert_eq!(buffer[(39, b_y)].symbol(), "┐");
        }
    }
    // Capping a widget inside a Grid must not change the equal track allocation.
    let mut grid = raw;
    grid["root"] = json!("composer");
    grid["elements"]["composer"]["type"] = json!("Grid");
    grid["elements"]["composer"]["props"] = json!({"columns":2,"gap":0});
    grid["elements"]
        .as_object_mut()
        .unwrap()
        .retain(|id, _| ["composer", "input", "send"].contains(&id.as_str()));
    let grid = Spec::parse(&grid.to_string()).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(100, 3)).unwrap();
    terminal
        .draw(|f| grid.render(f, f.area(), None, 0))
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(49, 0)].symbol(), "┐");
}

#[test]
fn explicit_widths_and_measured_bounds_match_native_rendering() {
    use ratatui::layout::Rect;
    let raw = json!({"root":"root","state":{"draft":"","show":false},"elements":{
        "root":{"type":"Column","props":{"grow":1,"width":"fill","widthPercent":20},"children":["body","composer","hidden"]},
        "body":{"type":"Row","props":{"gap":1,"collapseBelow":60,"grow":1},"children":["rail","log"]},
        "rail":{"type":"Panel","props":{"title":"Rail","width":16},"children":["label"]},
        "label":{"type":"Text","props":{"text":"Navigation"}},
        "log":{"type":"ScrollView","props":{"title":"Conversation","height":4,"text":"Hello","width":"fill","grow":1}},
        "composer":{"type":"Row","props":{"gap":1},"children":["input","send"]},
        "input":{"type":"Input","props":{"label":"Message","value":{"$bindState":"/draft"},"width":"fill"}},
        "send":{"type":"Button","props":{"label":"发送","width":"content"},"on":{"press":{"action":"setState","params":{"statePath":"/draft","value":""}}}},
        "hidden":{"type":"Text","props":{"text":"Hidden"},"visible":{"$state":"/show"}}
    }});
    let spec = Spec::parse(&raw.to_string()).unwrap();
    for (width, height) in [(80, 24), (120, 36), (160, 48), (40, 24), (1, 1)] {
        let report = spec.layout_report(width, height);
        let nodes = &report["nodes"];
        assert!(nodes.get("hidden").is_none());
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), None, 0))
            .unwrap();
        if width > 1 {
            assert_eq!(nodes["root"]["bounds"][2], width);
            assert_eq!(
                nodes["send"]["bounds"],
                json!([width - 8, height - 3, 8, 3])
            );
            assert_eq!(
                nodes["input"]["bounds"],
                json!([0, height - 3, width - 9, 3])
            );
            assert_eq!(
                spec.focus_offset_in("send", Rect::new(0, 0, width, height)),
                height - 3
            );
            assert_eq!(
                terminal.backend().buffer()[(width - 8, height - 3)].symbol(),
                "╭"
            );
            assert_eq!(
                terminal.backend().buffer()[(width - 10, height - 3)].symbol(),
                "┐"
            );
            if width >= 60 {
                assert_eq!(nodes["rail"]["bounds"][2], 16);
                assert_eq!(nodes["log"]["bounds"][0], 17);
                assert_eq!(nodes["log"]["bounds"][2], width - 17);
            } else {
                assert_eq!(nodes["log"]["bounds"][0], 0);
                assert_eq!(nodes["log"]["bounds"][2], width);
            }
        }
    }
    // Fit a padded container around display-cell widths, including wide/combining glyphs.
    let panel = json!({"root":"panel","elements":{
        "panel":{"type":"Panel","props":{"title":"","padding":2,"width":"content","horizontalAlign":"center"},"children":["text"]},
        "text":{"type":"Text","props":{"text":"界e\u{301}"}}
    }});
    let panel = Spec::parse(&panel.to_string()).unwrap();
    assert_eq!(
        panel.layout_report(40, 20)["nodes"]["panel"]["bounds"][2],
        9
    );
    let grid = json!({"root":"grid","elements":{
        "grid":{"type":"Grid","props":{"columns":2,"gap":1,"width":"content"},"children":["a","b"]},
        "a":{"type":"Text","props":{"text":"a"}},
        "b":{"type":"Text","props":{"text":"bbbb"}}
    }});
    let grid = Spec::parse(&grid.to_string()).unwrap();
    assert_eq!(grid.layout_report(40, 20)["nodes"]["grid"]["bounds"][2], 9);
    for invalid in [
        json!(0),
        json!(241),
        json!(-1),
        json!(1.5),
        json!(true),
        json!("automatic"),
    ] {
        let mut bad = raw.clone();
        bad["elements"]["rail"]["props"]["width"] = invalid;
        assert!(Spec::parse(&bad.to_string()).is_err());
    }
}

#[test]
fn ansi_art_measures_decoded_lines_including_borders() {
    let art = json!({"root":"art","elements":{"art":{"type":"AnsiArt","props":{"title":"Banner","height":4,"content":"\\u001b[36mTOP\\nONE\\nTWO\\nBOTTOM\\u001b[0m"}}}});
    let spec = Spec::parse(&art.to_string()).unwrap();
    assert_eq!(spec.content_height(), 6);
    let mut terminal = Terminal::new(TestBackend::new(30, 6)).unwrap();
    terminal
        .draw(|f| spec.render(f, f.area(), None, 0))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let line: String = (0..30).map(|x| buffer[(x, 4)].symbol()).collect();
    assert!(line.contains("BOTTOM"));
    assert!(
        buffer
            .content
            .iter()
            .any(|cell| cell.symbol() == "B" && cell.fg == ratatui::style::Color::Cyan)
    );
}
