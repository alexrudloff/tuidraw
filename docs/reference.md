# TUI Draw reference

For the short introduction, start with the [README](../README.md). This page keeps the detailed controls, file formats and implementation notes.

Build terminal applications through a persistent chat or an agent-facing CLI. A cheap model proposes primitive widgets, state, operations and targeted edits. Code applies and validates them, previews the interface in Ratatui, and exports a standalone Rust project. Follow-up prompts edit the same app by default.

The workflow takes inspiration from [Lovable's conversational builder and source export](https://lovable.dev/lenny). This version generates local UI behavior; application backends belong in the exported action hook.

## Start a conversation

Requires Rust 1.88+ and Node 22+ (tested with Rust 1.94.1 and Node 24.4.1).

```sh
cd tuidraw
nvm use
npm ci
cargo run
```

Start at a centered, bordered project picker. Select a project with the arrow keys or mouse, then use the Open project, New project, Add existing…, or Delete… buttons. Tab cycles buttons; Enter activates them; Ctrl+D deletes the selected project. Delete shows Cancel/Delete buttons with Cancel initially focused and moves the entire folder, including custom source and history, into a sibling `.tui-draw-trash/` folder. Use Undo delete to restore the most recent deletion while the menu is open; restoration refuses an occupied path. After leaving the menu, files remain recoverable by moving them back from trash. Projects currently being written cannot be deleted. The launcher originated in TUI Draw, was refined after a usability review, and runs through its shared widget renderer; see [the embedded UI](../ui/README.md).

Launcher buttons show direct accelerators: Alt+N New, Alt+A Add existing, Alt+O Open, Alt+D Delete, Alt+U Undo. Dialogs use Alt+C Cancel, Alt+N Create, Alt+O Open, and Alt+Y to confirm deletion. Existing Ctrl+N/O/D shortcuts and Esc still work. Alt shortcuts require the terminal to send Alt/Option as an escape prefix; Tab and Enter remain available.

Generated buttons have a `hotkey` property, rendered beside their label and included in width measurement. The model can request an accelerator; code fills missing ones and rejects collisions. Bindings survive refinement and use the same handlers in the preview and exported app, including custom action hooks. Ordinary typing does not trigger them; hidden, disabled and background-modal buttons cannot fire. Old saved specs without hotkeys remain compatible and acquire assignments on their next generated edit.

The preview uses the full terminal except for a one-line footer. **Ctrl+G** opens a chat box over the bottom of the interface. Describe an interface, then give feedback: “make the status area compact,” “use violet and gold,” or “add a Next turn button.” Hiding chat preserves your draft and never resizes the preview. Replies stream into chat with tool progress. Questions save to conversation history without creating a UI version. A completed edit saves one undoable version. While it works, keep typing and press Enter to queue follow-up messages; they run after the current request finishes. Cancellation or failure pauses the queue; Enter resumes it. Queued messages are kept only while the app is open.

| Key / command | Action |
| --- | --- |
| Ctrl+G | Open or hide chat |
| Esc | Hide chat or cancel a project picker; a build keeps running |
| Ctrl+P | Return to the project menu |
| Enter in chat | Send a message; queue it if the agent is busy |
| Tab / Shift+Tab | Close chat and select preview controls; cycle controls in preview |
| Enter / Space on a control | Activate it locally |
| Ctrl+N / Ctrl+O | New project / open a saved project folder |
| Ctrl+S | Save preview state and generated source locally |
| Ctrl+Z | Restore unsaved preview changes, otherwise undo the last saved version |
| Ctrl+X | Cancel a running build, retaining the current interface and draft |
| PgUp / PgDn | Scroll conversation when chat is open, preview otherwise |
| ↑ in chat | Recall the last submitted prompt |
| Ctrl+U | Clear the prompt or folder field |
| /new [folder], /open [folder] | Create or open a project from chat |
| /save, /undo, /export, /help | Local project commands |
| Ctrl+C | Save preview changes and quit |

Run `tui-draw` after installing with `cargo install --path .`, or use `cargo run`. `tui-draw chat DIR` opens a project's chat directly. The internal Rust library and existing config/index paths retain `ratatui-json` for compatibility, so prior projects and model settings continue to work.

Relative paths resolve against the current project's parent folder (shown in the picker); `~` is supported. Saved-project recents live in `$XDG_STATE_HOME/ratatui-json/projects.json` (default `~/.local/state/ratatui-json/projects.json`); project content stays in its chosen directory. Input cursor movement, deletion, Unicode and pasting are supported. Selectors, sliders, tabs and scroll views use their native arrow keys. Preview interactions are local; a successful feedback turn includes the current preview state. Ctrl+S, switching projects and normal quit persist changed preview state. The headless `export` command exports the saved version. Errors and cancellations retain the previous screen and the draft prompt.

Numeric displays support `color` and `thresholds` on Metric, Gauge, Sparkline and BarChart. Colors can be theme roles (`primary`, `success`, `warning`, `danger`, `foreground`, `muted`), literal `green`/`yellow`/`red`, or `#RRGGBB`. Example: `"color":"green","thresholds":[{"min":70,"color":"yellow"},{"min":90,"color":"red"}]`. Bounds are inclusive and strictly increasing; the highest matching bound wins. Metrics/gauges follow their current value, bars are colored individually, and sparklines use the latest sample for the whole series. Numeric strings and a trailing `%` work for metrics. These are reusable rules selected by the model, not GPU-specific behavior.

## Agent-facing CLI

```sh
cargo build --release
./target/release/tui-draw build "A modern TradeWars interface" --out ./trade --json
./target/release/tui-draw edit ./trade "Put navigation beside the sector map" --json
./target/release/tui-draw inspect ./trade --json
./target/release/tui-draw layout ./trade --width 120 --height 36 --json
./target/release/tui-draw undo ./trade --json
./target/release/tui-draw chat ./trade

# Import a validated spec without any model call:
./target/release/tui-draw build --spec examples/primitives.json --out ./controls --json

# Run the generated app independently:
cargo run --manifest-path ./trade/Cargo.toml
cargo run --manifest-path ./trade/Cargo.toml -- --check
cargo run --manifest-path ./trade/Cargo.toml -- --snapshot
```

`--json` writes one JSON result to stdout and progress to stderr. Failure returns `{ "ok": false, "error": "..." }` with a nonzero exit code. Success includes project/source paths, revision, summary and generation metrics. `inspect` also includes the current spec and turn history. Quote prompts as one argument. `RATATUI_JSON_NODE` can select an explicit Node executable.

`layout` reports each visible widget's `bounds`, panel `inner` bounds, and allocated `slot` as `[x,y,width,height]`, using the same native layout as drawing and keyboard focus. Coordinates are in the scrollable document before scrolling, not the final popup overlay. `build` and `edit` also accept `--width` / `--height` (default 100×36); chat uses its actual preview viewport. No model call is needed to inspect layout.

The model receives the current measured layout with each feedback request. It can declare bounded `geometryChecks` for gap ranges, width/height ranges, and edge alignment to an ancestor or viewport. Native measurements validate the proposed result before saving. Failed checks return actual dimensions for the existing single repair attempt; that repair cannot relax the checks. `minViewportWidth` limits a relationship to the intended responsive breakpoint. Checks and the resulting geometry are saved in turn metrics. These check declared spatial requirements, not aesthetic quality or whether the model captured every user intention.

Native reports also contain `audits`: hard errors for empty visible areas and controls too small to use, and warnings for artwork clipping or text contrast below 4.5:1. Errors feed the existing one-repair path before saving; warnings remain inspectable in layout/turn metrics. Ordinary document scrolling is allowed. These checks run even when the model supplies no geometry assertions.

Shared styling is saved in `theme.design`, for example:

```json
{"border":"double","density":"compact","emphasis":"bold"}
```

Ask for “rounded borders and quiet headings” or “compact double-line BBS styling” in ordinary feedback. Border choices are plain, rounded, double and heavy; density is compact or comfortable; emphasis is quiet or bold. Containers/panels/tables inherit spacing and padding. Set their gap/padding/border properties to `null` to inherit or use explicit values to override. All other bordered primitives inherit the border family. Artwork retains its authored colors. Old projects continue to render with legacy defaults until given shared styling; `/export` updates their managed runtime.

Each project contains:

- `src/ui.rs`: actual Rust construction code for the interface, with stable widget IDs.
- `src/actions.rs`: a create-once custom button hook. Match an ID and return `Ok(true)` to replace the generated action, or `Ok(false)` to run it. Agent-written code survives edits and undo. This hook runs in the exported application; the builder preview runs the generated local actions.
- `src/main.rs` and `Cargo.toml`: a create-once keyboard runner and manifest, available for customization.
- `runtime/`: a self-contained native widget/runtime crate. The exported app needs no Node, LLM or Jev service.
- `ui.json`: the current validated design, suitable for inspection or starting another project.
- `.builder/session.json`: successful versions, prompts, summaries, state, model usage and timings.

Generated files changed outside the builder cause an export conflict; the builder does not overwrite them. Back up those edits and restore the generated version before retrying. User-owned files are never regenerated. Undo restores generated design/state and preserves custom source; editing after undo replaces the abandoned future branch. History is bounded to 64 versions and a 16 MB compact session; import `ui.json` into a new project to continue beyond that limit. Concurrent writers are rejected. After a forcibly killed write, remove `.builder/write.lock` only after confirming no writer remains. File replacement is atomic per file, not a cross-file transaction; an interrupted filesystem write can leave generated files needing restoration before `export` can reconcile them with the saved session. The builder validates UI definitions; compile/test custom Rust changes yourself.

## Models, Jev and performance

CLI and chat share a small native tool-calling loop inspired by Pi's control flow, without adding Pi as a dependency. The model can answer directly, call `inspect_interface` for the current spec and measured bounds, request missing uncommon primitive contracts with `lookup_widgets`, or apply a batch through `edit_interface`. Sixteen core contracts are preloaded with shared sizing properties declared once. Edits use the existing compiler, binding/action validation, hotkey assignment and native geometry audits. The edit tool does not invoke another content LLM. Related edits should be batched to avoid one round trip per widget. Code assigns safe internal IDs for newly declared names that violate the identifier format and rewrites their references consistently, without a model repair. Existing valid IDs, visible labels and state paths stay unchanged; duplicate declarations and missing targets still fail validation. Chat displays short repair statuses while the model receives the detailed diagnostics.

Successful assistant/tool exchanges persist separately from UI revisions. The next request receives the current interface outline and recent complete exchanges; detailed props are available through inspection. Old projects load without migration steps. Context is bounded by dropping whole oldest exchanges, preserving tool-call/result pairs. The UI keeps 128 exchanges and 64 revisions. Undo affects the interface; it does not erase conversation history.

A request has a 120-second deadline, at most six model responses and twelve tool calls. Distinct validation failures share that turn budget; a separate three-error counter no longer cuts off a repair that is making progress. The visible interface is unchanged during generation. If a draft can be assembled but fails widget or layout validation, it stays private: the agent can inspect it and send small corrective edits without regenerating the screen. Widget validation reports errors across the batch together, including the state path, resolved type and required property contract for invalid bindings. Repeating an invalid pending widget keeps that diagnostic instead of masking it as a no-op. Schema or tree failures retain the raw proposal for small `repair_proposal` JSON Pointer patches, so one misplaced field does not force a complete regeneration. Accepted batches stay in the temporary draft. Literal color thresholds are sorted by code; duplicate limits and malformed values still fail validation. Refinement spatial requirements survive repair. For new interfaces, the agent can revise its own provisional layout checks while finding a design that fits. Native reports include each container’s minimum width for controls and identify limiting fixed-width ancestors, so corrections account for siblings, borders and padding. The draft is committed after a successful `final:true` edit, a normal assistant finish, or a turn limit reached with a validated draft, as one revision. Cancellation, model truncation, unresolved validation failures and limits reached before a valid edit retain the saved interface. If the turn budget ends with a validated draft and no pending edit error, that draft is saved with an explicit turn-limit notice; a seventh model response is not required just to acknowledge success. Failed agent runs save their completed tool exchanges and model metrics to `.builder/last-failure.json` for diagnosis; this local file can include the prompt and UI data and is replaced by the next failure. Conversation-only replies do not export or overwrite source. Undo, export and preview interaction make no model calls.

Property edits use `updates:[{id,key,value}]` for scalars, arrays, objects, and bindings. The older `updates:[{id,key,json}]` JSON-encoded form remains accepted. Only that property is replaced; other properties, state bindings, events and child lists remain intact. `state` declares defaults without overwriting existing values; explicit `stateUpdates:[{key,value}]` sets or creates the named keys. The completed widget still passes ordinary schema, action and native checks. This keeps threshold, table-data and column-style edits compact. Malformed JSON, unsafe keys and excessive nesting are rejected as tool errors for the agent to repair. See [the edit latency comparison](edit-benchmark.md).

Jev is **enabled by default** for build, edit, chat, and launching without arguments. Pass `--engine llm` to build, edit or chat to use only the content model. For short, unambiguous layout edits, code enumerates and natively validates candidate changes, then Jev selects one or abstains; a selected change needs no content-model call. For clipped controls, Jev can select among native-validated width, spacing and stacking repairs. Within `edit_interface`, Jev also selects shared design settings for new interfaces or explicit theme edits and can resolve uncertain numeric operands in newly proposed button actions. Explicit state references stay fixed. Ordinary chat and unrelated edits make no Jev calls. The deterministic compiler applies the selected palette and chrome before native measurement, so repair sees the final style. The older experimental json-render layout composer remains available through `bridge/compose.mjs` for comparison and its offline demo. Its extra layout calls are not on the default builder path.

The [paired design benchmark](design-benchmark.md) found Jev faster at isolated style selection but less accurate on a light-to-dark edit. Jev is enabled by user preference. The LLM-only path selects chrome within its existing content call, so the isolated timing difference does not establish an end-to-end saving. Compare separate fresh projects and inspect per-turn metrics: total bridge time including repairs, model calls and token usage, plus Jev grounding time and decisions. Also compare actual operation behavior, layout and edit preservation; schema validity alone is not quality. Unknown pricing is not reported as zero or as savings.

With `OPENAI_API_KEY`, the main content model defaults to GPT-5.6 Terra with reasoning disabled. Settings load from `~/.config/ratatui-json/config.json`; environment values override them, and `LLM_MODEL=gpt-5.6-luna` restores the previous model. No keys are embedded in project exports. The default endpoint is `https://api.openai.com/v1`; OpenAI credentials are sent only to that exact HTTPS origin, and redirects are rejected.

Without an OpenAI key, the default is the local DGX at `http://100.115.205.43:8000/v1`, model `latest`. Override with `LLM_BASE_URL=http://brains.local:8000/v1`; `LLM_API_KEY` authenticates custom providers. Local requests use low thinking, a 512-token thinking budget and 4,096 total output tokens. OpenAI output is capped at 4,096 tokens, with reasoning disabled for GPT-5 models. Tool arguments are validated against the primitive catalog and bounded edit schema. A session can contain up to 128 widgets, depth 8, 1 MB per spec and 4,096 content rows.

Jev accepts `JEV_ENDPOINT`, `JEV_MODEL`, `JEV_API_KEY`, or Vercel's `JEV_AI_GATEWAY_API_KEY` / `AI_GATEWAY_API_KEY`. Its optional grounding call has a 15-second timeout. Prompts, current UI state and recent feedback are sent to the selected model; Jev receives only the bounded decision context when invoked. UI state also appears in saved history and exported source.

## What is implemented

Every primitive accepts `props.grow` (integer 0–16; leaves default to 0, containers inherit the largest visible child weight). In a `Column` or `Panel`, children keep their minimum heights and divide extra viewport height by this weight. Set `grow:1` on expanding content and its ancestor containers; leave headers and bottom controls at 0. Explicit `grow:0` stops inheritance for a fixed-height section. A growing `ScrollView` with `height:4` fills available space while retaining a four-row minimum. Rows pass their height to children; grid rows share extra space by their largest child weight. The same layout runs in the preview and exported Rust app.

Presentation options are native and model-selectable:

- Every widget: `width:24` requests 24 terminal cells, `width:"content"` fits measured text/controls (including borders), and `width:"fill"` shares the remaining Row space. For example, use a fixed sidebar beside a filling main column, or a filling input beside a content-sized button. Unicode uses terminal display-cell widths. Fluid charts have no intrinsic horizontal size and still fill their slot. Fixed/content widths shrink when the viewport cannot accommodate them.
- `width:null` or omission preserves legacy sizing. `maxWidth` (0 = unlimited, up to 240 cells) remains an upper bound. `widthPercent` (1–100) scales inside the allocated slot only when width is null; `horizontalAlign` (`left`, `center`, `right`) positions a narrower widget inside its slot. Legacy capped Row children consume at most `maxWidth` cells and uncapped siblings share the rest. Grid tracks remain equal; collapsed Rows stack vertically. `grow` controls vertical expansion only.
- `Column`, `Row`, `Panel`: `gap` (0–4). `Row.collapseBelow` stacks its children below a chosen width; 0 disables collapse. `Grid.minCellWidth` reduces the column count to fit, with `columns` as the maximum; `gap` sets cell spacing. Existing grids preserve their original spacing when omitted.
- `Panel`: `border` (`plain`, `rounded`, `heavy`, `double`, `none`), `padding` (0–4), and `titleAlign`. Borderless panels can group content without another box.
- `Select.searchable`: type a case-insensitive fuzzy query, navigate with arrows, and Enter commits the highlighted option. Typing alone never changes the bound value. Backspace edits the query; Tab changes focus.
- `MultiSelect`: binds an array of unique option strings; Space toggles the highlighted item. With `searchable:true`, typing filters while selections outside the filter remain selected. Both selectors accept up to 100 options. Search text/cursor are transient; committed values persist. `setState` with `[]` clears multiple selections.
- `Table`: `columnStyles` is empty for defaults or has one `{width,align,color}` per header. Width 0 shares remaining space; positive widths are terminal cells. Colors use theme roles (`foreground`, `muted`, `primary`, `success`, `warning`, `danger`). `columnGap`, cell `padding`, `showHeader`, and `border` control density and presentation.

Try the reusable primitives without a model call:

```sh
cargo run -- build --spec examples/adaptive-controls.json --out /tmp/adaptive-demo
cargo run -- chat /tmp/adaptive-demo
```

The example stacks on narrow terminals and includes searchable destinations, a cargo checklist, and a formatted market table. These are ordinary composed primitives, not a game-specific widget. Existing saved specs retain their defaults. Regenerate an older project's managed runtime with `export`; its custom `src/main.rs` remains yours. A custom runner should use `content_height_in(viewport_width)` and `focus_offset_in(id, viewport)` so scrolling follows responsive layout.


The catalog now has **28 live-generation component types**. Layout/content: `Column`, `Row`, `Grid`, `Panel`, `Text`, `Metric`, `Table`, `List`, `Badge`. Controls: `Input`, `Button`, `Switch`, `Select`, `MultiSelect`, `Tabs`, `Slider`, `Popup`, `ScrollView`. Visuals: `Gauge`, `Sparkline`, `BarChart`, `Chart`, `Equalizer`, `BigText`, `PlayingCards`, `QRCode`, `AnsiArt`, `Scene`. Rust resolves `$state` and `$bindState`, supports state-based visibility, and implements local `setState` and `compute` actions. Inputs and actions validate changes before accepting them. The sample Save button changes the status to “Saved locally”; it does not write to disk or a service.

### Graphics and expanded native widgets

The [tui-widgets collection](https://github.com/ratatui/tui-widgets) supplies big/box text, playing cards, QR codes, equalizers, popups, scroll views and braille bar graphs. Big/box text share `BigText.font`; the braille graph is `Sparkline.dense`. Existing inputs and scrollbars cover overlapping upstream functionality. Charts, tabs, selectors and sliders use Ratatui primitives. Selectors, sliders, tabs, popups and scrolling react locally without model calls.

`Scene` renders a colored ASCII tile map with a symbol/color legend. Rows may be ragged; the renderer pads and centers them, doubles horizontal spacing when it fits, and uses muted gray for unlisted characters. Maps are limited to 64 columns by 24 rows and 12 legend entries. `AnsiArt` renders multiline ANSI illustrations with 16-color, 256-color and RGB styles using [ansi-to-tui](https://github.com/ratatui/ansi-to-tui). It supports up to 16,000 characters and a declared minimum height of 4–26 rows. Native measurement expands to fit decoded artwork plus borders, subject to the overall 4,096-row limit. Style escapes become Ratatui cells; terminal commands are not replayed. These are visual surfaces, not animation or movement engines.

Game generation instructions request a prominent scene or illustration alongside relevant controls. The model supplies compact content; native code draws it; the agent chooses the layout. Try `Build a dungeon crawler with a room map, party status and inventory`, or `Build a Tradewars interface with a sector map and destination selector`.

```sh
cargo run -- --spec examples/widget-gallery.json
cargo run -- --spec examples/ansi-starship.json
cargo run -- --spec examples/dungeon-scene.json
```

### Native recipes

Button variants, semantic badge colors, switches and shared control theme roles are native adaptations of [tuiparts](https://github.com/tuiparts/tuiparts), with attribution in `THIRD_PARTY_NOTICES.md`. This is the initial port of those controls, not the entire OpenTUI library.

With `--engine llm`, the content model chooses a complete `theme` palette for every new interface; the default Jev path selects bounded design axes and code derives the palette: `background`, `surface`, `foreground`, `muted`, `border`, `primary`, `focus`, `danger`, `success`, and `warning`, each a `#RRGGBB` color. The palette travels with every preview snapshot and saved spec. Refinements and validation repairs can return `theme: null` to preserve it; requests to recolor can supply a new palette. Native controls, borders, text and charts share these roles, with readable foreground selection on filled controls. Older specs retain a default palette. Authored artwork and functional QR/card/equalizer colors keep their own colors.

`Grid {columns}` is a generic container. The agent places independent primitives into its cells; new screens preserve the proposal's row-major reading order in code. Inputs, displays and buttons are separate components; there is no new dice-roller component or hardcoded dice UI. The old `Keypad` renderer remains available only for imported legacy specs, including `examples/calculator.json`; it is excluded from live generation.

Buttons support one action or an ordered array of up to eight actions. `setState` assigns a literal or a state reference. `compute` accepts `{statePath, op, args}`. Arithmetic (`add`, `subtract`, `multiply`, `divide`) takes two operands; `append` joins two values; `backspace` removes the last character; `evaluate` evaluates a bounded arithmetic string. `randomInt` takes `[minimum, maximum, count]` and sums independent inclusive random samples (count 1–32). Arguments can read current state. Results preserve numeric targets or format as text for string targets. A press commits atomically; invalid arithmetic, bounds or resulting UI state leave all prior state intact. None of these actions call a model.

For example, a generated dice interface can combine ordinary inputs, a result Text and a Button that samples into `/result`, then adds the modifier. The same operations support counters, numeric utilities and calculators. `Text` and `Metric` display string, number or boolean state. The bridge rejects buttons with no observable path to a display/control and placeholder strings masquerading as state references, both before composition and on the completed tree.

```sh
cargo run -- --spec examples/primitives.json
cargo run -- --spec examples/jev-primitives.json  # captured live LLM + Jev build
```

`bridge/catalog.mjs` owns the primitive contracts and allowed actions. `bridge/agent.mjs` runs the streamed conversation/tool loop; `bridge/generate.mjs` shares proposal validation with the older comparison generator; `bridge/document.mjs` applies its edits to a copy and checks the tree. `bridge/build.mjs` is the common bridge used by both CLI and chat. `bridge/design.mjs` defines bounded design axes and deterministic palettes; optional `bridge/ground.mjs` connects uncertain numeric operands with Jev. Semantic intent and layout remain model decisions; schema validity alone cannot prove either.

`src/lib.rs` is the reusable renderer. Parse with `Spec::parse` or construct with `Spec::from_parts`, then call `render(frame, area, focused_id, scroll)`. `src/project.rs` saves versions; `src/export.rs` emits Rust; `src/cli.rs` exposes headless commands; `src/main.rs` provides the interactive host. `bridge/compose.mjs` retains upstream `experimental_composeSpec` and newline-delimited events for legacy composition experiments.

## Verify

```sh
cargo test --locked
node --test bridge/*.test.mjs
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
python3 tests/restart.py
python3 tests/builder.py
python3 tests/projects.py
cargo run -- --spec examples/ansi-starship.json --snapshot
node bridge/compose.mjs --demo
```

Native tests cover rendering, theme, controls, actions, rollback, session persistence, undo, concurrent edits and source conflicts. Bridge tests cover deterministic patches, preservation across turns, state changes, tree rejection, model context and both Jev paths. `tests/builder.py` checks the CLI and resumed chat with a local fake model, compiles exported Rust, and exercises its keyboard runner without calling any model service.

## Experimental dependency

The json-render Jev exports are unreleased. `vendor/json-render-core-0.21.0.tgz` comes from upstream commit `3ad381881194e7011ad3ccd6d668033495a06c29`, not the published package. Its Apache-2.0 license is included; see `vendor/README.md`. The native renderer, bounded edit format and Rust exporter live in this repository.
