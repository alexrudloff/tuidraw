# TUI Draw

## Register
product

## Users
Developers build terminal applications through a conversational interface with a live, keyboard-usable preview. Coding agents invoke the same engine through a noninteractive CLI and consume its generated source, diagnostics and changes. Both entry points operate on the same resumable project/session.

## Product Purpose
A conversational terminal-application builder: describe, preview, give feedback, and export usable code. Lovable is the workflow reference. The first output target is Rust/Ratatui applications. The existing validated JSON representation and native primitives support previews and deterministic code generation; the target deliverable includes editable source, dependencies, state and action integration points. Exported applications run independently of the builder's model services. Do not add a domain-specific component to fix a composition failure.

Implemented: resumable project sessions, targeted feedback turns, local undo, an agent-facing CLI with JSON results, and standalone Rust source export with a preserved custom-action hook. The first version retains bounded local UI actions; it does not generate an application backend or merge arbitrary hand edits into generated layout source.

## Brand Personality
Practical, compact, legible. The user selected tuiparts as the component reference and specifically rejected a calculator assembled as unrelated, full-width buttons.

## Anti-references
Generic stacks of oversized bordered boxes, calculator keypads scattered by a model, static buttons pretending to perform arithmetic, and fixed saved-prompt routing.

## Design Principles
Provide reusable layout, display, input and operation primitives. Use Grid for conventional spatial relationships and explicit behavior contracts/state dependencies. Preserve the always-accessible chat overlay and support arbitrary requests. Feedback should update the current project by default, preserve working behavior and agent-written logic, and create reversible versions. Keep machine-readable CLI output separate from progress messages. The host opens to a centered project menu with New/Open/Delete and saved projects. Editing presents a full canvas and toggleable bottom chat overlay; project controls and save status remain accessible. New projects start with an empty state; demo-prompt function keys do not belong in the working interface.

Use a cheap LLM plus deterministic validation/compiler as the baseline. Jev is enabled by default by explicit user preference (2026-09-21); `--engine llm` remains the comparison path. Its end-to-end speed and quality benefit is not established. Measure successful completion, behavior, rendering, edit preservation, latency including retries, and model cost; do not equate valid JSON with a successful application. Avoid extra model calls for local interactions, undo, saving, validation or code emission.

## Accessibility & Inclusion
Keyboard navigation, visible focus and selected states, labels in addition to semantic color, no animation requirement, and graceful clipping on tiny terminals. These follow existing host behavior and the referenced control recipes.
