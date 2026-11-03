# Personal Niri maintenance

This checkout belongs to the AMD HawkPoint laptop `nlh-cachy` and uses
`machine/amd-laptop`. Verify the actual host, branch, remotes, worktree and merge
state before editing or pushing. The NVIDIA desktop `cachyos` is a separate
machine; do not merge, mirror or deploy its machine branch here.

This is a complete Niri source branch, based on `StatIndet/niri-edge`, with the
local changes already committed. Do not reapply `niri-desktop.patch` or the old
Smithay workaround. Do not add NVIDIA retained-frame, FP16, full-repaint,
refresh-threshold, GPU-pinning or power-policy changes to this branch.

Read `docs/PERSONAL-MAINTENANCE.md` before updating upstream or deploying.
`origin` is the personal repository, `upstream` is `StatIndet/niri-edge`, and
`official` is `niri-wm/niri`. Preserve their history. Push only the current
machine's branch when requested; do not force-push or mirror upstream branches.

Preserve the local floating focus behavior, independent application/panel blur,
HiDPI Genie fixes and titlebar double-click behavior. Respect existing work and
use an isolated review branch for upstream merges. Test the changed behavior and
compile before deployment. Existing titlebar checks are documented in the
maintenance guide; do not add source-text or implementation-shape tests.

The paired desktop configuration repository is
`/home/nlh/.local/share/desktop-config`, on the matching machine branch.
`sources.lock.json` pins the integrated Niri source commit. Preserve unrelated
changes in it and in Clavis; never capture or commit another task's work.
Fetching or merging source does not authorize replacing the running compositor
or ending the user's session.
