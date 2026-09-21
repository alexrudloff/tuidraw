use ratatui::{Terminal, backend::TestBackend, style::Color};
use ratatui_json::Spec;
use serde_json::json;

#[test]
fn numeric_colors_follow_live_values_and_each_bar_without_changing_geometry() {
    let thresholds = json!([{"min":70,"color":"yellow"},{"min":90,"color":"red"}]);
    let mut raw = json!({"root":"root","state":{"value":69},"elements":{
        "root":{"type":"Column","props":{},"children":["metric","gauge","spark","dense","bars"]},
        "metric":{"type":"Metric","props":{"label":"Utilization %","value":{"$state":"/value"},"color":"green","thresholds":thresholds}},
        "gauge":{"type":"Gauge","props":{"title":"Load","value":{"$state":"/value"},"color":"green","thresholds":thresholds}},
        "spark":{"type":"Sparkline","props":{"title":"Trend","data":[20,70],"color":"green","thresholds":thresholds}},
        "dense":{"type":"Sparkline","props":{"title":"Dense","data":[20,90],"dense":true,"color":"green","thresholds":thresholds}},
        "bars":{"type":"BarChart","props":{"title":"Nodes","labels":["A","B","C"],"values":[69,70,90],"color":"green","thresholds":thresholds}}
    }});
    let mut spec = Spec::parse(&raw.to_string()).unwrap();
    let bounds = spec.layout_report(60, 40)["nodes"].clone();
    for (value, color) in [
        (69, Color::Rgb(34, 197, 94)),
        (70, Color::Rgb(234, 179, 8)),
        (89, Color::Rgb(234, 179, 8)),
        (90, Color::Rgb(239, 68, 68)),
    ] {
        spec.state["value"] = json!(value);
        let mut terminal = Terminal::new(TestBackend::new(60, 40)).unwrap();
        terminal
            .draw(|f| spec.render(f, f.area(), None, 0))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 1)].fg, color);
        for (id, expected) in [
            ("gauge", color),
            ("spark", Color::Rgb(234, 179, 8)),
            ("dense", Color::Rgb(239, 68, 68)),
        ] {
            let r = bounds[id]["bounds"].as_array().unwrap();
            let y = r[1].as_u64().unwrap() as u16;
            let h = r[3].as_u64().unwrap() as u16;
            assert!(
                (y + 1..y + h - 1).any(|row| (1..59).any(|col| buffer[(col, row)].fg == expected)),
                "{id}"
            );
        }
        let y = bounds["bars"]["bounds"][1].as_u64().unwrap() as u16;
        for c in [
            Color::Rgb(34, 197, 94),
            Color::Rgb(234, 179, 8),
            Color::Rgb(239, 68, 68),
        ] {
            assert!((y + 1..40).any(|row| {
                (1..59).any(|col| buffer[(col, row)].fg == c && buffer[(col, row)].symbol() != " ")
            }));
        }
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.bg == Color::Rgb(234, 179, 8) && cell.fg == Color::Black)
        );
        assert_eq!(spec.layout_report(60, 40)["nodes"], bounds);
    }
    spec.state["value"] = json!(70);
    assert!(Spec::parse(&serde_json::to_string(&spec).unwrap()).is_ok());
    for bad in [
        json!([{"min":90,"color":"red"},{"min":70,"color":"yellow"}]),
        json!([{"min":0,"color":"not-a-color"}]),
    ] {
        raw["elements"]["metric"]["props"]["thresholds"] = bad;
        assert!(Spec::parse(&raw.to_string()).is_err());
    }
    raw["elements"]["metric"]["props"]["thresholds"] = thresholds;
    raw["elements"]["metric"]["props"]["value"] = json!("87%");
    assert!(Spec::parse(&raw.to_string()).is_ok());
    raw["elements"]["metric"]["props"]["value"] = json!("87%%");
    assert!(Spec::parse(&raw.to_string()).is_err());
}
