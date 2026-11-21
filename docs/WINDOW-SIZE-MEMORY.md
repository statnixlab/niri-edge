# Remember application window size

Companion to `remember-window-layout` (tiled vs floating). Enable with:

```kdl
layout {
    remember-window-layout
    remember-window-size
}
```

For each application (exact app-id) two sizes are remembered independently:

- the floating window size (logical pixels), and
- the tiled column width, stored as a proportion of the working area so it
  adapts to other outputs.

Only normal state is learned: maximized, fullscreen, windowed-fullscreen and
full-width columns are ignored. Tiled window heights are not remembered.

When several windows of one application are open, the most recently *changed*
window wins (resized, column width changed, or switched between tiled and
floating). A window that merely opens with a different size does not replace an
existing record. This matches the policy of `remember-window-layout`.

On open, the remembered size takes precedence over `default-column-width` and
`default-window-height`, which stay the fallback when there is no record.

Eligibility matches layout memory: unparented, resizable windows that are not
forced fullscreen. Window rules adjust it:

```kdl
// Exclude windows that size themselves to content, e.g. video players.
window-rule {
    match app-id="^mpv$"
    remember-window-size false
}

// Keep an auxiliary window's sizes separate from the main windows.
window-rule {
    match app-id="^example$" title="^Add Download$"
    window-memory-group "add-download"
}
```

`remember-window-size true` in a rule forces inclusion even when the layout
option is off or the window is a dialog. A group changes the record key to
`<app-id>#<group>`.

State is stored in `$XDG_STATE_HOME/niri/window-sizes.json`, normally
`~/.local/state/niri/window-sizes.json`. It is written when a window closes,
2 seconds after the last change, and at orderly shutdown, using the same atomic
background writer as layout memory. Invalid or newer-format files are preserved
and persistence is disabled for that instance.

The size is applied in the initial configure, so rules that select a group or an
exclusion must already match then.
