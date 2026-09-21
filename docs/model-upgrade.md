# Main content model comparison

On 2026-09-21, the current agent was run with GPT-5.6 Luna and GPT-5.6 Terra on the same three tasks, twice each, with model order reversed on the second pass. Both used `reasoning_effort:"none"`, Jev, native validation, and a 110×35 viewport. The title edit used the same saved IRC fixture. Raw per-run results and returned specs are local under `~/local-delegate-evals/2026-09-21-builder-speed/model-upgrade/`; neither includes credentials.

| Task | Luna | Terra |
| --- | --- | --- |
| Rename title | 2/2 valid; 1.2–2.2 s | 2/2 valid; 1.6–2.0 s |
| New TradeWars screen with visual area and buttons | 1/2 valid; 9.2–12.9 s | 1/2 valid; 23.8–33.3 s |
| Calculator with a real button grid | 1/2 valid; 13.6–16.5 s | 2/2 valid; 14.5–15.2 s |

Terra passed 5/6 checks with 11 content calls and five rejected tool results; Luna passed 4/6 with 20 content calls and 16 rejected results. Both Terra calculators were built in one content call with working arithmetic buttons. Luna needed five calls for its one successful calculator. The TradeWars result is mixed, so this small sample does not establish a general quality advantage.

Total elapsed time was 90.5 s for Terra versus 55.7 s for Luna. Estimated GPT API cost was $0.144 versus $0.017, using [Terra](https://developers.openai.com/api/docs/models/gpt-5.6-terra) and [Luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna) token rates and reported cached tokens; reported Jev costs are separate in the raw results. Terra is now the OpenAI default for the main content model. Set `LLM_MODEL=gpt-5.6-luna` to use the prior model. The comparison checks structure, button presence and native layout, not visual appeal or full game semantics.
