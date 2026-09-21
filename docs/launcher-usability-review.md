# Launcher usability review

Expert inspection, 2026-09-21. Scope: current launcher render, generated spec, native event handlers, and the supplied OpenCode references. No new observed-user study; scores are provisional judgments, not an official Nielsen measurement. UI unchanged.

The interface gives too much visual attention to its controls relative to the simple task of opening or starting a project. The component defaults are still determining the composition. The result looks assembled and overly ceremonial, despite working keyboard and mouse mechanics.

Framework: [Nielsen's usability heuristics](https://www.nngroup.com/articles/ten-usability-heuristics/). Two independent reviews covered visual design and source-backed interaction evidence. The HTML/CSS detector is inapplicable to native Ratatui; no automated visual-quality score is claimed.

## Provisional score

Scale: 0 poor, 4 excellent. Total **27/40**: usable foundation, significant design refinement needed.

| Principle | Score | Evidence |
|---|---:|---|
| Status feedback | 3 | Selection and selected path visible; action feedback displaces help. |
| Familiar language | 3 | Most labels familiar; Open/Open folder distinction ambiguous. |
| Control and exits | 3 | Cancel and Escape available; trash recovery requires manual file work. |
| Consistency | 3 | Functional controls; static Enter hint does not follow focus. |
| Mistake prevention | 3 | Cancel is initially focused; empty-state actions disabled; deletion checks locks. |
| Visible choices | 3 | Real labeled actions and project list; no path memorization for known projects. |
| Efficiency | 3 | Arrow/Enter and mouse support; no project filtering. |
| Visual economy | 2 | Repeated headings, nested framing and excessive action emphasis. |
| Recovery | 2 | Files recoverable, but no visible Restore/Undo action. |
| Help | 2 | Basic hints; context-dependent Enter behavior and post-delete hint loss. |

## Priority findings

1. **P2: Weak action hierarchy.** Delete's saturated fill competes with Open. Four equally sized buttons mix selected-project actions (Open/Delete) with global actions (New/Add existing). Group by scope; emphasize opening, offer New distinctly, keep Delete visible but quiet until confirmation. Commands: impeccable layout/clarify.
2. **P2: Too much framing and repetition.** “Your projects,” “Choose a project to open or manage,” and “Projects · 3” repeat the same message. The list frame inside the dialog frame adds another enclosure for the same task. Keep one border, one heading, and a compact brand mark. The thin large logo and heavy controls currently feel like different visual styles. Commands: impeccable distill/typeset.
3. **P2: Ambiguous Open folder action.** The label may mean reveal the selected folder in a file manager or browse the filesystem. Actual behavior opens a typed-path form for another saved project. “Add existing project…” better expresses intent; do not call it Browse unless it actually provides browsing. Command: impeccable clarify.
4. **P2: Focus hints misdescribe the action.** “Enter open” stays visible after Tab focuses New/Delete. Enter then activates the focused button. Update the hint with focus, or use “Enter activate.” Numeric project prefixes also resemble shortcuts although typing their numbers does not select a project. Remove numbers unless they provide a useful interaction. After deletion, status overwrites the only shortcut row indefinitely. Keep feedback separate from controls. Commands: impeccable clarify/harden.
5. **P2: Recovery is technically possible but operationally awkward.** Confirmation promises recovery via .tui-draw-trash, yet the interface provides no Restore or Undo action. Keep Cancel-default confirmation and offer Undo after deletion. Command: impeccable harden.

## What works

Selection has both a color fill and a marker. The selected path helps distinguish similarly named projects. Opening requires only arrows and Enter. Keyboard, mouse and cancellation support provide a sound foundation; these should survive simplification.

## Cognitive load and user perspectives

Moderate avoidable load: grouping and hierarchy fail the eight-item checklist; the remaining items broadly pass for this small choice set. The four buttons alone are not excessive, and counting every project plus every action as simultaneous decisions would exaggerate the problem.

A newcomer must interpret Open versus Open folder. A frequent user can open quickly but cannot filter a long list. A user recovering from a mistaken deletion must leave the app and manipulate files. Screen-reader usability was not evaluated.

## Proposed structure

One centered bordered picker. Compact TUI Draw title. New project and Add existing near the heading, separate from selection-specific controls. Projects occupy the main space. Selected path beneath the list. Open project is primary; Delete remains a compact secondary button at the opposite end. Focus-aware keyboard hints sit in a stable footer. Keep branding, but give the working list greater priority.

The dogfood lesson is that a valid generated composition is only one acceptance criterion. Clear action scope, priority and task completion still need evaluation. Feed these principles into reusable primitives and generation guidance instead of hand-polishing only this one screen.
