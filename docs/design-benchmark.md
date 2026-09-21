# Shared terminal design: paired benchmark

Measured 2026-09-21; [raw results](design-benchmark.json). Six requests, three repeats per engine, sequential requests with engine order reversed on alternate repeats. Both select the same independent axes. Four explicit new styles, one unrelated edit that must preserve styling, and one explicit light-to-dark restyle. All outputs were rendered through native layout measurement at 80×24 and 120×36 using the same generic panel/metric/input composition.

| Selector | Whole requests correct | Median request time | Range | Input/output tokens | Reported API cost |
| --- | ---: | ---: | ---: | ---: | ---: |
| gpt-5.6-luna | 18/18 | 1,042.5 ms | 799–2,387 ms | 6,480 / 618 | Unknown |
| typesafe/jev-1.13-20260917 | 15/18 | 374.5 ms | 292–515 ms | 13,956 / 3,984 | $0.000586152 |

Jev retained the existing light canvas in all three repeats of an explicit dark-cyan restyle; its other axes were correct. All compiled themes passed the native fit checks. Costs are gateway-reported, not inferred from tokens. Failed sandbox probes and an earlier invalid test fixture/request shape were harness failures and are excluded from these results; they are retained in the local evaluation directory.

**Original benchmark recommendation: keep the LLM default and Jev opt-in.** The user subsequently chose Jev as the app default; `--engine llm` remains available. Jev is faster at this small selection task, but did not improve quality here. The LLM-only path chooses shared chrome alongside content in its existing call; adding a separate selector cannot be assumed to reduce total build time. These explicit checks test instruction adherence and renderability, not subjective beauty, broad domain quality, or statistical significance. Three repeated runs do not establish a general win.

Reproduce (uses configured model endpoints and incurs model calls):

```sh
cargo build --offline
RATATUI_JSON_NATIVE="$PWD/target/debug/ratatui-json" node bridge/design-bench.mjs /tmp/design-benchmark.json 3
```

A separate live Jev build produced an IRC interface with shared double borders, an exact 20-cell rail, filling conversation and bottom composer in 7,341 ms (523 ms design selection, one 6,814 ms content call), with no native audit findings. This is one successful smoke test, not a paired end-to-end speed comparison.

A live LLM-engine follow-up changed the same project to light rose, rounded chrome and quiet headings in 3,203 ms, with one content call. All element definitions, actions, state and native bounds remained identical. The contrast audit reported one warning (muted text on surface: 4.43:1); the edit was saved without an extra model call. Standalone export compiled and rendered without Node or model access.
