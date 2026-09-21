# TUI Draw draws TUI Draw

`launcher.json` is the embedded startup UI. Its original version was generated with the default Jev + GPT-5.6 Luna pipeline and refined through `edit`. The usability review then guided a manual composition refinement using the same generic Panel, Row, Select, Button and Text primitives available to generated apps.

The picker has one titled frame, compact global actions above a borderless list, the selected path, a primary Open action and a quiet Delete action. `Button.variant: "plain"` provides one-row bracketed buttons; `Select.bordered: false` omits an unnecessary inner frame. Both retain their previous rendering by default. No network request happens at startup.

Host integration replaces sample project labels/path, disables Open/Delete when empty, updates hints for the focused control, and keeps feedback separate. Button commands trigger native validated filesystem operations. Every button declares a visible `hotkey`; Alt+N/A/O/D/U cover New/Add/Open/Delete/Undo, while dialogs expose Alt+C Cancel and Alt+N/O/Y for Create/Open/Delete confirmation. Delete defaults to Cancel, moves source/history to sibling trash, and offers Undo for the most recent deletion while this menu is open. Restoration refuses an occupied destination or a live writer. Older deletions remain recoverable from `.tui-draw-trash/`.

The shared usability instructions in `bridge/design.mjs` reach content generation, edits, repair and Jev design/composition without an extra model pass. They describe action priority, task grouping, restrained framing, truthful affordances and real feedback/recovery, without domain-specific templates.

To refine the launcher yourself:

```sh
tui-draw build --spec ui/launcher.json --out /tmp/tui-draw-launcher
tui-draw chat /tmp/tui-draw-launcher
```

Keep stable IDs `projects`, `project-path`, `instruction`, `status`, `open`, `new`, `browse`, `delete`, `undo`, and state keys `selectedProject`, `projectPath`, `command`, `status`, `hasStatus`, `canUndo`. Button commands are `open`, `new`, `browse`, `delete`, `undo`. Review `/tmp/tui-draw-launcher/ui.json` and copy it back here to embed the result. Filesystem actions belong to the launcher host; a standalone generated preview only changes command state.
