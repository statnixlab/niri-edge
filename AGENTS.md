# NVIDIA desktop Niri maintenance

This checkout is for the RTX 4070 SUPER desktop `cachyos` and uses
`machine/nvidia-desktop`. Verify the actual host, hardware, branch, remotes,
worktree and merge state before editing or pushing. The AMD laptop uses a
separate branch; do not merge or mirror either machine branch into the other.

The complete Niri source and this desktop's local patches are committed here.
`origin` is `statnixlab/niri-edge`, `upstream` is `StatIndet/niri-edge`, and
`official` is `niri-wm/niri`. Read `docs/PERSONAL-MAINTENANCE.md` before updates.
Preserve history and review upstream merges on a separate branch. Push only the
requested machine branch, without force or mirror operations.

Preserve this desktop's retained-frame/Smithay #2151 integration, refresh-aware
workaround controls, floating focus stacking, native floating-window maximize,
application/panel blur and HiDPI Genie correction. Keep its display, GPU, power,
suspend and application compatibility settings independent of the AMD laptop.
Do not import the AMD titlebar double-click change unless explicitly requested.

Smithay is a fixed Git dependency from `statnixlab/smithay` on its NVIDIA machine
branch. Cargo.toml and Cargo.lock pin the integrated patch commit. Do not reapply
Niri or Smithay patches or substitute an unpatched Smithay checkout.

The paired `/home/nlh/.local/share/desktop-config` repository has the same
machine branch and pins the complete Niri/Smithay commits in sources.lock.json.
Preserve unrelated local and Clavis work; do not capture another task's changes.
Validate source/dependency changes before deployment. Fetching, saving and
publishing source do not authorize restarting or replacing the running desktop.
