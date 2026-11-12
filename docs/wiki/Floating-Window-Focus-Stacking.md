# Floating-window focus stacking

```kdl
layout {
    floating-windows-follow-focus
}
```

This opt-in setting lowers the currently raised floating windows when a scrolling
window is explicitly activated. Each floating window records its own position
relative to the scrolling layer. Opening a new floating window or activating one
specific floating window raises that window, without raising previously lowered
unrelated windows. Its child dialogs follow it so they remain above their parent.

Closing the last raised float returns focus to the scrolling layer instead of
bringing back older lowered windows. Floating windows keep their size, position
and workspace and are not minimized. Exposed parts remain clickable; covered
windows can be raised through a dock, window activation, or
`switch-focus-between-floating-and-tiling`.

The setting is disabled by default and supports per-workspace layout overrides.
Focus-following pointer motion does not raise windows by itself. Rendering,
closing animations, pointer hit testing and resize-edge targeting consistently
partition windows as raised floats, scrolling windows, then lowered floats.
