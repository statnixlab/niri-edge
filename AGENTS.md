# NVIDIA desktop Niri maintenance

This checkout is for the RTX 4070 SUPER desktop `cachyos` and uses
`machine/nvidia-desktop`. Verify the host, branch, remotes, worktree and
rebase state before editing or pushing. The AMD laptop uses a separate
branch; never merge or mirror either machine branch into the other.

The branch is official niri (`official` = `niri-wm/niri`) plus a short stack
of personal patches, one commit each. Read `docs/PERSONAL-MAINTENANCE.md`
before updating. Updates rebase the stack onto `official/main` on a review
branch, after tagging the previous state as `archive/nvidia-desktop-YYYYMMDD`.
Do not merge StatIndet/niri-edge; it is reference only.

Preserve native minimization with Genie/scale animations (Clavis depends on
it), the retained-frame/Smithay #2151 integration, refresh-aware workaround
controls, floating focus stacking, native floating-window maximize,
application/panel blur, HiDPI Genie correction and titlebar double-click.

Smithay is a fixed Git dependency from `statnixlab/smithay` on its NVIDIA
machine branch: official niri's Smithay revision plus the retention patch.
Do not substitute an unpatched Smithay.

The paired `/home/nlh/.local/share/desktop-config` repository pins the Niri
and Smithay commits in `sources.lock.json`. Publish Smithay first, then Niri,
then the configuration lock. Push rebased branches only with
`--force-with-lease`, after the archive tag is pushed, and only when asked.
Building, saving or publishing source does not authorize restarting or
replacing the running desktop.
