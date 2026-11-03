# AMD laptop Niri source

This branch is a complete, buildable source tree. It contains the history of
`StatIndet/niri-edge` and the AMD laptop's local changes as ordinary Git commits.
There is no separate patch-application step. It belongs to `nlh-cachy`, an AMD
HawkPoint laptop; NVIDIA-specific code and configuration remain independent.

## Repositories and branches

| Remote | Repository | Purpose |
| --- | --- | --- |
| `origin` | `https://github.com/statnixlab/niri-edge.git` | Personal source; push `machine/amd-laptop` |
| `upstream` | `https://github.com/StatIndet/niri-edge.git` | niri-edge features and updates |
| `official` | `https://github.com/niri-wm/niri.git` | Official Niri updates |

The local checkout is `/home/nlh/.local/share/niri-source`. Both remote histories
are retained so Git can find the common ancestor and perform normal merges.
The initial niri-edge baseline is `b2aaeb3c619610827577e4fbbb3225ce88d19baa`.
The local commits preserve application blur control, floating focus stacking,
HiDPI Genie geometry, titlebar column behavior and double-click recognition.

## Reviewing updates

Verify the machine identity and preserve all existing work first:

```sh
hostnamectl --static
git status --short --branch
git remote -v
git fetch --multiple upstream official
git log --oneline HEAD..upstream/main
```

When an update is requested, create a review branch or worktree based on
`machine/amd-laptop`. Usually merge `upstream/main` to retain niri-edge's
integration work. Official updates can instead be merged from `official/main`
when explicitly selected; review their compatibility with native minimization
and the local changes. Avoid rebasing or squashing the already published history.

```sh
git switch -c review/niri-update-YYYYMMDD machine/amd-laptop
git merge upstream/main
```

Resolve conflicts by preserving the intended behavior, not by replacing the
tree with either side. Inspect and validate the resulting diff before merging
the review branch into `machine/amd-laptop`. The NVIDIA machine branch must not
be used as an update source. Port an individual reviewed fix with attribution
only when requested and validated for this machine.

## Building and validation

Build the checked-out source directly; its Cargo.lock already pins Smithay:

```sh
cargo build --release --locked --bin niri --jobs 6
```

For changes to the titlebar and configuration behavior, the relevant checks are:

```sh
cargo test --release --locked -p niri --lib titlebar
cargo test --release --locked -p niri-config --lib
```

Run other existing tests when an update affects their behavior. Verify the new
binary with `niri validate --config <intended-config>` before selecting it for
the next login. Build into a separate target directory when sharing a checkout
with another active task. Do not use the old NVIDIA patch/build flow from the
desktop configuration repository.

## Version alignment and deployment

The matching `machine/amd-laptop` branch in `statnixlab/desktop-config` pins this
branch's complete commit in `sources.lock.json.niri`. Its `source_mode` is `git`;
the source preparer checks out that commit without applying additional patches.
After saving a source change, update that pin without collecting unrelated
desktop or Clavis work. Push the Niri source before a configuration commit that
references it. Source review, source publication and binary activation are
separate operations.

The binary installed on 2026-11-03 was built from the same source content as the
titlebar change commit, before this checkout was created. Its build metadata
retains that provenance. Creating this branch does not rebuild or restart Niri.
Keep the working binary and its startup entry available when deploying a newer
build, and activate the new binary at an agreed session boundary.
