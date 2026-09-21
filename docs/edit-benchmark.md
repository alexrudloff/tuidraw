# Compact edits and model comparison

Measured 2026-09-21. [Raw final results](edit-benchmark.json).

| Path | Correct completed edits | Median round trip | Slowest round trip | Repair calls | Output tokens, including repairs |
| --- | ---: | ---: | ---: | ---: | ---: |
| Previous Luna path | 9/12 | 4,137 ms | 8,053 ms | 4 | 3,274 |
| Compact patches + Luna | 12/12 | 2,865 ms | 3,612 ms | 0 | 1,831 |
| Compact patches + GPT-4.1 Mini | 12/12 | 3,150 ms | 4,088 ms | 2 | 2,228 |

**Kept compact patches with Luna.** Median round-trip time fell 31%; output tokens fell 44%, including repair traffic. Among successful requests only, the baseline median was 4,022 ms, so the successful-only reduction was 29%. Jev stays enabled by default. The content model remains `gpt-5.6-luna`; no private model settings were changed. New-interface generation was not optimized or benchmarked here.

The change extends existing property updates to arrays and objects, using `{id,key,json}` with a JSON-encoded value. It replaces one property, preserving the rest of the widget. The model no longer needs to repeat a whole metric/table to change thresholds, rows, or column styles. Scalar `{id,key,value}` updates remain supported. Full widget, state, action and native layout validation still runs; malformed/unsafe/deep JSON is rejected, and only one repair is permitted. Generation metrics now include each content request's elapsed time.

## Method and limits

Four tasks, three repeats, with model/implementation order rotated each repeat:

- Apply green/yellow/red thresholds to both a GPU metric and its bar chart.
- Replace alert table rows while preserving headers and styling.
- Change three table column widths while preserving alignment and colors.
- Widen an IRC channel rail to 26 cells while its workspace fills the remainder.

Each request starts from an immutable copy of the same saved dashboard or chat. Jev selection, content calls, repairs, native validation and measurement are included in round-trip timing. These are bridge timings; they exclude host startup, file export and terminal repaint. Runs are sequential, not competing requests. No user project is changed.

Correctness checks assert the requested property values or measured geometry and preservation of all unrelated normalized element properties, bindings, event actions, state and theme. Equivalent threshold representations (e.g. a green min:0 entry) are accepted. Source snapshots and proposals remain in the local evaluation directory. The baseline's three final failures were invalid nested `props` edits; they remain in the timing and failure counts. Final runs had no infrastructure errors.

An initial pass exposed missing base-color updates and prompted clearer generic instructions. The final run above uses those instructions. Earlier pilot/intermediate results, including a Jev timeout, remain separately in the evaluation directory. This is a small development benchmark on two interfaces, not a held-out quality evaluation or a promise of the same improvement for every prompt.

Warm prompt-cache reads occurred in all variants. No cache settings were changed. Gateway-reported Jev cost for the final run was $0.000449946 per variant ($0.001349838 total); GPT cost was not reported and remains unknown. Token counts are workload measurements, not measured dollar savings. The local review's tokens/cost accounting is separate.

## Reproduction

Use Node 22+ and build the native binary. Supply a frozen baseline bridge directory and an output directory containing immutable `dashboard.json` and `chat.json` fixtures with the IDs referenced by the harness. This uses configured model credentials and makes paid calls.

```sh
cargo build --offline
RATATUI_JSON_NATIVE="$PWD/target/debug/ratatui-json" node bridge/edit-bench.mjs OUTPUT_DIR BASELINE_BRIDGE_DIR 3
```

The original baseline, fixtures and detailed outputs are under `~/local-delegate-evals/2026-09-21-edit-latency/`. The harness writes results incrementally and saves successful specs and model proposals, including failed repair proposals. API keys and request headers are never written to the reports.

Verification: 28 bridge tests passed, including complex-property repair, object bindings, unsafe/deep JSON rejection and source immutability. The CLI/chat/export smoke test also passed, including rollback, undo, preserved custom Rust and the standalone keyboard runner.
