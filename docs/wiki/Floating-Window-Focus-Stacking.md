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

To make dialogs follow their tiled or floating owner as a group, also enable
`floating-windows-follow-parent`. See [Floating window families](../FLOATING-WINDOW-FAMILIES.md)
for parent discovery, conservative app-id fallback, fullscreen behavior and limitations.

## Floating-window top boundary

```kdl
layout {
    floating-windows-match-tiling-top
}
```

This optional setting keeps ordinary floating tiles below the same top edge as
normal tiled windows. Both use the compositor's current panel-reserved work area,
the layout's top strut and outer gap, with the same output-scale rounding.
There is no separate numeric floating boundary to keep in sync.

Changing Clavis bar thickness, edge margin or exclusive-zone offset updates this
edge through layer-shell, including during an active drag. Removing the reserved
area (for example, hiding the bar or enabling overlay mode) updates both layouts.
Horizontal and bottom movement retain their existing limits.

The constraint applies to command moves, placement, resize updates, normal-view
pointer dragging, and floating movement animations. Overview drag previews stay
free to move between workspace thumbnails; the dropped window is constrained in
the destination workspace. Native maximize and fullscreen retain their existing
behavior. The setting is off by default and supports output/workspace overrides.

## Titlebar double-click while floating

With `titlebar-double-click-to-column` enabled, double-clicking a floating
window's titlebar toggles expansion into the same usable rectangle as a normal
full-width tiling column. The window remains in the floating layer throughout;
it does not advertise native Maximized or Fullscreen state. Double-click again
to restore its saved size and position. Each window stores its own restore data.

The expanded rectangle follows current panel reservations, struts, gaps and
scale. It respects application size constraints. Saved restore geometry travels
with the window across minimization and workspace/output moves. Native maximize
buttons and programmatic maximize requests retain their existing behavior;
restoring from native maximization returns to the expanded float.

Dragging an expanded float restores its ordinary size under the pointer.
Explicit move/resize commands leave expansion mode. The ordinary floating top
boundary continues to follow the tiling layout when configured.

## Drag floating windows to half-screen tiling

```kdl
layout {
    floating-windows-edge-snap
}
```

Dragging a floating window into the outer 8 logical pixels at either horizontal
output edge shows a half-screen insertion preview. Release there to create a
normal half-width tiling column aligned to that side. Moving the pointer away
before release cancels the preview and keeps the window floating. The preview
and placement respect the destination output's work area, struts, gaps and scale.
Application size constraints remain authoritative.

The action uses the pointer position, not the window edge. Existing tiled windows
remain in their columns. The window's floating size is retained for a later
floating toggle. Overview drags and ordinary tiled-window drags keep their normal
behavior. Snap previews at corners suppress simultaneous workspace edge scrolling.
The feature is disabled by default and supports output/workspace layout overrides.
