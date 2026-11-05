# NVIDIA desktop source maintenance

This complete source branch belongs to `cachyos` (RTX 4070 SUPER), independently
of the AMD laptop. It starts at niri-edge commit
`b2aaeb3c619610827577e4fbbb3225ce88d19baa` and contains the desktop's existing
local modifications as ordinary Git commits. It is not a merge of the AMD
machine branch. On 2026-11-04, the titlebar double-click change was ported
individually from AMD commit `599ad91ca34a556db8702d780e93621effda0c7f`.
Enable `layout { titlebar-double-click-to-column; }` to keep double-click column
sizing while maximize buttons use native maximization. The NVIDIA Smithay
dependency and display workarounds remain unchanged.

## Source repositories

| Remote | Repository | Purpose |
| --- | --- | --- |
| `origin` | `https://github.com/statnixlab/niri-edge.git` | Personal `machine/nvidia-desktop` branch |
| `upstream` | `https://github.com/StatIndet/niri-edge.git` | niri-edge updates |
| `official` | `https://github.com/niri-wm/niri.git` | Official Niri updates |

The checkout is `/home/nlh/.local/share/niri-source`. Full history is retained so
Git can find the common ancestors when merging upstream updates.

The NVIDIA-only Smithay dependency is maintained separately at
`https://github.com/statnixlab/smithay.git`, on `machine/nvidia-desktop`, with a
checkout at `/home/nlh/.local/share/smithay-source`. It preserves the deployed
Smithay PR #2151 framebuffer-retention patch on baseline
`79bbed5e1199090d787115614847a79c76607181`. Cargo.toml and Cargo.lock pin its
complete commit, so builds do not need an external patch-application step.

## Reviewing updates

Verify identity and existing work, then fetch:

```sh
hostnamectl --static
git status --short --branch
git remote -v
git fetch --multiple upstream official
git log --oneline HEAD..upstream/main
```

When an update is requested, create a review branch/worktree from
`machine/nvidia-desktop`, then normally merge `upstream/main`. `official/main`
can be used when direct official updates are specifically selected. Resolve
conflicts without losing native minimization, local window behavior or the
NVIDIA display workarounds. Do not rebase or squash already published history.

```sh
git switch -c review/niri-update-YYYYMMDD machine/nvidia-desktop
git merge upstream/main
```

Smithay updates are reviewed in its own source repository. Preserve the
retained-frame patch or confirm that an upstream equivalent replaces it before
changing the Niri dependency. Port individual reviewed fixes from another
machine only when requested, retain attribution, and validate them here.

## Building and validation

Build the integrated source directly:

```sh
cargo build --release --locked --bin niri --jobs 4
```

Choose checks for the affected behavior. For titlebar and floating stacking:

```sh
cargo test --release --locked -p niri --lib titlebar
cargo test --release --locked -p niri --lib floating_stacking
```

Validate the intended config with the new binary before selecting it for a
future session. A successful build does not by itself validate a GPU workaround;
display changes need verification on this NVIDIA machine at its normal refresh
rate. Do not end a desktop session just to organize or save source history.

## Pins and publication

The matching desktop-config branch pins Niri and Smithay in sources.lock.json.
Niri uses `source_mode: git`, and Smithay is resolved from Cargo.lock. Publish
Smithay first when its code changes, then Niri, then the configuration lock.
The preparer preserves existing dirty work and rejects a different branch or
commit instead of resetting it. Full desktop capture is a separate operation.

During this migration, all tracked Niri code was compared with the retained
2026-10-29 `niri-publish` build input, and all Smithay code with its patched
dependency. Only dependency locations/pins and maintenance documentation were
changed. The installed desktop binary SHA-256 was
`68fc0a0d0c5af6105cd1475e1491eb701341f1070ba85392b28e1595f27befa3`.
It remains selected by the existing startup configuration.
