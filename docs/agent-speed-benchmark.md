# Agent speed comparison

Measured 2026-09-21 against a frozen copy of the earlier bridge. Both paths used the configured lightweight content model, default Jev mode, native validation, the same immutable IRC fixture and the same 110×35 viewport. Two sequential runs per task, with variant order rotated. Raw measurements remain local under `~/local-delegate-evals/2026-09-21-builder-speed/final/results.json`; the benchmark script is [speed-bench.mjs](../bridge/speed-bench.mjs).

| Task | Earlier path | New path | Result |
| --- | ---: | ---: | --- |
| Widen channel rail to 26 cells | 4.2, 7.6 s | 0.4, 0.5 s | Exact width, preserved other widgets and state in both runs |
| Rename application title | 2.3, 2.9 s | 1.2, 1.6 s | Exact title, preserved other widgets and state in both runs |
| New TradeWars interface | 16.3, 22.4 s | 14.9, 15.5 s | New path included visual art, controls and native-valid layout in both runs; earlier path omitted art in one |

The new rail edit used no content-model calls; Jev chose one prevalidated patch. The title edit used one content-model call, versus two earlier. New-interface output varied: the new path took two and six content calls, and the earlier path three and four. Validated Jev options for stacking cramped control rows and combining independent width and row repairs were added after these paired measurements. A subsequent 100×35 TradeWars run completed in 10.8 seconds with one content-model call and one successful Jev layout repair; that unpaired run is not a comparable speed measurement.

These are two samples per task, not a general latency estimate. The TradeWars acceptance check requires Scene or AnsiArt, a Button, no native layout errors, and a saved draft. It does not judge the interface's aesthetics or game mechanics. The first earlier TradeWars run passed; the second produced a schema-valid interface without the requested visual area. The measured new path was 6/6 on this task set; the earlier path was 5/6 by those checks. Build startup and source export are outside the bridge timing. Jev's gateway reported hosted cost separately in raw results; GPT cost and net cloud savings are unmeasured.
