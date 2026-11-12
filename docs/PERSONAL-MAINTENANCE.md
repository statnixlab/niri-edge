# NVIDIA desktop source maintenance

This branch belongs to `cachyos` (RTX 4070 SUPER). Since 2026-11-12 it is
**official niri plus a short stack of personal patches**. It no longer follows
StatIndet/niri-edge; the history before the restructure is kept at the tag
`archive/nvidia-desktop-20261112`.

## Patch stack

Each patch is one commit directly on top of `official/main`, in this order:

| Patch | Purpose |
| --- | --- |
| Native window minimization with Genie and scale animations | Squashed from StatIndet/niri-edge. Required by Clavis (Dock minimize, Genie/Scale setting, `Niri.minimizeWindow`). |
| NVIDIA desktop window behavior and display compatibility | Refresh-aware flicker controls, full repaint, 16-bit output, floating focus stacking, native floating maximize, panel/app blur, HiDPI Genie correction. |
| Titlebar double-click and native maximization | `layout { titlebar-double-click-to-column; }`. |
| Pin the NVIDIA Smithay fork | Cargo.toml/Cargo.lock point at `statnixlab/smithay`. |
| Maintenance documentation | This file and `AGENTS.md`. |

Keep each change in its own commit. A new personal change is a new commit at
the top of the stack (or a fixup of the patch it belongs to).

## Source repositories

| Remote | Repository | Purpose |
| --- | --- | --- |
| `official` | `https://github.com/niri-wm/niri.git` | Base for the patch stack |
| `origin` | `https://github.com/statnixlab/niri-edge.git` | Personal `machine/nvidia-desktop` branch |
| `upstream` | `https://github.com/StatIndet/niri-edge.git` | Reference only, for minimization fixes |

Smithay lives in `/home/nlh/.local/share/smithay-source`
(`statnixlab/smithay`, `machine/nvidia-desktop`) and is maintained the same way:
the Smithay revision pinned by official niri plus the framebuffer-retention
patch from Smithay PR #2151.

## Updating to a newer official niri

```sh
hostnamectl --static                 # must be cachyos
git status --short --branch          # must be clean
git fetch official
git tag archive/nvidia-desktop-$(date +%Y%m%d) machine/nvidia-desktop
git switch -c review/official-$(date +%Y%m%d) machine/nvidia-desktop
git rebase official/main
```

Conflicts are reported per patch. Resolve them inside that patch, keeping
the behavior described above. If official niri changed its Smithay revision:

1. In smithay-source, create a branch from the new revision and
   `git cherry-pick -x` the framebuffer-retention patch.
2. Update the revision in the "Pin the NVIDIA Smithay fork" patch and run
   `cargo update -p smithay -p smithay-drm-extras`.

Drop a patch when official niri gains an equivalent feature.

## Building and validation

```sh
cargo build --release --locked --bin niri --jobs 4
cargo test --release --locked -p niri-config
cargo test --release --locked -p niri --lib titlebar
cargo test --release --locked -p niri --lib floating_stacking
cargo test --release --locked -p niri --lib minimize
./target/release/niri validate -c ~/.config/niri/titlebar-native-buttons.kdl
```

Before the Smithay commit is published, build with a local mapping:

```sh
CARGO_NET_GIT_FETCH_WITH_CLI=true GIT_CONFIG_COUNT=1 \
GIT_CONFIG_KEY_0=url./home/nlh/.local/share/smithay-source.insteadOf \
GIT_CONFIG_VALUE_0=https://github.com/statnixlab/smithay.git \
cargo build --release --locked --bin niri --jobs 4
```

Install a new binary next to the old ones in `~/.local/lib/niri-desktop/`
under a dated name, then point the systemd drop-in at it. A successful build
does not validate a GPU workaround; display changes need verification on this
machine at its normal refresh rate. Do not end the desktop session to save or
publish source.

## Publishing

Push Smithay first, then Niri, then the desktop-config lock. Because the
stack is rebased, push the machine branch with `--force-with-lease` only after
the archive tag for the previous state has been pushed.
