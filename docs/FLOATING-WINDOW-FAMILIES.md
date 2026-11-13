# Floating windows following their owner

This personal option extends the existing floating-focus stacking patch:

```kdl
layout {
    floating-windows-follow-focus
    floating-windows-follow-parent
}
```

Both options must be enabled. The new option defaults to off, so existing
configurations keep their behavior. It does not require per-application rules.

When a main window or one of its dialogs is activated, its floating family
is raised together, with children above parents. Activating another family
lowers the previous family's floating windows behind the tiling layer.
Keyboard focus remains on the requested window. Geometry, client mapping,
minimization and workspace placement are not changed. Render order and pointer
hit testing use the same layer. An inactive parent's newly opened non-focused
dialog is lowered as well.

Ownership is resolved within each workspace:

1. Use declared `xdg_toplevel` parent relationships, including nested dialogs.
2. For an unparented floating root only, use its nonempty exact app-id if there
   is exactly one tiled window with that app-id on this workspace. This is a
   heuristic for clients that omit parent information, not a protocol guarantee.
3. With multiple matching main windows, no app-id, or a declared parent outside
   the workspace, do not guess. Independent floats retain the existing stacking
   behavior. The fallback cannot distinguish an independent floating document
   from a utility window with the same app-id and one tiled document.

The floating root and its descendants always share a layer. Parent, app-id,
focus and topology changes are re-evaluated during layout refresh. Explicit
focus-without-raising requests preserve the current stacking, including during
subsequent refreshes.

This implements activation-based family stacking. It does not clip a dialog
to the parent's rectangle, move it with the parent, automatically move/minimize
the family, or provide an always-on-top exception for picture-in-picture.
It applies to managed toplevel windows; layer-shell surfaces and ordinary popup
surfaces are not reclassified. X11 applications depend on the parent/app-id
information forwarded by Xwayland's window manager.

Validation uses layout regression tests and an isolated headless compositor
with real xdg_toplevel parent/app-id requests. It does not control or restart
the user's desktop.

```sh
cargo test --release --locked -p niri --lib floating_stacking --jobs 4
cargo test --release --locked -p niri-config --jobs 4
cargo build --release --locked --bin niri --jobs 4
```
