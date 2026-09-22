# TUI Draw

**Describe a terminal app. Refine it in chat. Export runnable Rust.**

TUI Draw turns a prompt into a working [Ratatui](https://ratatui.rs) interface. A content model chooses the widgets and actions; [Jev](https://github.com/vercel-labs/json-render) makes bounded design decisions. Every edit is validated in the native renderer before it becomes a saved version. The exported app runs without Node or a model service.

## Try it

Requires Rust 1.88+ and Node 22+. Set `OPENAI_API_KEY` and a Jev gateway (`JEV_ENDPOINT`, plus `JEV_API_KEY` if needed, or `JEV_AI_GATEWAY_API_KEY`).

```sh
git clone https://github.com/alexrudloff/tuidraw.git
cd tuidraw
npm ci
cargo run
```

Create a project in the menu, press **Ctrl+G**, and ask for something like “a TradeWars-style command deck with a sector map.” Then try “put navigation in a narrow left rail.” **Ctrl+P** returns to projects; **Ctrl+S** saves; **Ctrl+Z** undoes. Press `/help` in chat for the rest.

No Jev gateway? `cargo run -- chat ./demo --engine llm` uses only the content model.

## Use it from an agent

```sh
cargo run -- build "A space trading dashboard" --out ./trade --json
cargo run -- edit ./trade "Put navigation on the left" --json
cargo run -- chat ./trade
cargo run --manifest-path ./trade/Cargo.toml
```

`build` and `edit` return machine-readable results with `--json`. The project keeps its design, conversation, undo history and generated Rust source. Local controls work in the preview and exported app; connect external services through the generated `src/actions.rs` hook.

[Full reference](docs/reference.md) · [Widget examples](examples/widget-gallery.json) · [Design notes](DESIGN.md)
