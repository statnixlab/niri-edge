# Remember application window layout

Enable this machine's application layout memory with:

```kdl
layout {
    remember-window-layout
}
```

Each application is identified by its exact, nonempty Wayland app-id (including
the identifier forwarded by Xwayland's window manager). Its most recently
changed normal layout becomes the default for future main windows. Existing
windows are not changed to match. Opening or closing another window of the same
application does not replace a saved choice merely because its layout differs.
For an application without a record, the first eligible main window establishes
the default using the existing opening rules.

Layout changes are recorded during refresh and before unmapping. Fullscreen,
maximized and windowed-fullscreen placement does not replace the remembered
normal layout. Minimization preserves the existing choice. Parented dialogs,
fixed-size windows and windows forced fullscreen by a rule are excluded.
Unparented auxiliary windows can be excluded explicitly:

```kdl
window-rule {
    match app-id="^example$" title="^Utility$"
    remember-window-layout false
}
```

For eligible main windows, memory takes precedence over `open-floating`, which
remains the fallback when no record exists. Native dialogs and excluded windows
retain their ordinary window rules. The NVIDIA installation retains the existing
WPS, chat auxiliary-window, Firefox picture-in-picture and Clavis settings
exceptions in its configuration wrapper.

State is stored in `$XDG_STATE_HOME/niri/window-layouts.json`, normally
`~/.local/state/niri/window-layouts.json`. Only app-ids and `tiled`/`floating`
values are written, without window titles, documents or process identifiers.
A single background writer coalesces updates and writes a private temporary file,
syncs it, atomically renames it, and syncs the containing directory. The GUI thread
does not perform these disk writes. Orderly shutdown drains pending writes;
changes are saved throughout the session rather than only during logout.
A power loss before a pending write completes can still lose that last change.

Invalid or newer-format state files are preserved and persistence is disabled
for that instance, with a warning. The preference still works in memory.
Headless and nested compositor instances do not use the user's state path.
Tests use temporary directories and isolated Wayland clients.

This remembers layout type only, not window size, position, workspace or which
document was open. New application IDs are separate preferences. Applications
must provide a stable app-id in time for initial window mapping.

Turning the option off restores ordinary opening behavior without deleting the
saved preferences. The configuration option does not migrate or share state
between machines. On first installation, recording starts in the next Niri
session; it cannot recover unrecorded layout changes from earlier sessions.
