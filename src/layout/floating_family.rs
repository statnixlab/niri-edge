//! Resolve window families without application-specific names or window-title rules.

use super::LayoutElement;

/// Return the layer of floating windows belonging to a family. Independent floats are omitted.
/// Relations are local to this workspace: never infer an owner on another workspace/output.
pub(super) fn family_layers<W: LayoutElement>(
    windows: &[(&W, bool)],
    active: Option<&W::Id>,
) -> Vec<(W::Id, bool)> {
    let mut parents: Vec<_> = windows
        .iter()
        .enumerate()
        .map(|(idx, (window, _))| {
            windows.iter().enumerate().find_map(|(other, (parent, _))| {
                (idx != other && window.is_child_of(parent)).then_some(other)
            })
        })
        .collect();

    let app_ids: Vec<_> = windows.iter().map(|(window, _)| window.app_id()).collect();
    for (idx, (window, floating)) in windows.iter().enumerate() {
        // A reported parent always wins, including one not present on this workspace.
        if !floating || parents[idx].is_some() || window.has_parent() {
            continue;
        }
        let Some(app_id) = app_ids[idx].as_deref().filter(|id| !id.is_empty()) else {
            continue;
        };
        let mut candidates = windows
            .iter()
            .enumerate()
            .filter_map(|(other, (_, floating))| {
                (!floating && app_ids[other].as_deref() == Some(app_id)).then_some(other)
            });
        if let Some(parent) = candidates.next() {
            // Do not guess between multiple documents, or create a cycle by adopting an ancestor.
            if candidates.next().is_none() && root(&parents, parent) != Some(idx) {
                parents[idx] = Some(parent);
            }
        }
    }

    let roots: Vec<_> = (0..windows.len()).map(|idx| root(&parents, idx)).collect();
    let active_root = windows
        .iter()
        .position(|(window, _)| Some(window.id()) == active)
        .and_then(|idx| roots[idx]);

    windows
        .iter()
        .enumerate()
        .filter_map(|(idx, (window, floating))| {
            let root = roots[idx]?;
            // A floating main window and its dialogs must also stay in the same layer.
            let is_family = parents[idx].is_some() || parents.contains(&Some(idx));
            (*floating && is_family).then(|| (window.id().clone(), Some(root) == active_root))
        })
        .collect()
}

fn root(parents: &[Option<usize>], mut idx: usize) -> Option<usize> {
    // The protocol rejects cycles, but keep layout code robust against malformed test clients.
    for _ in 0..parents.len() {
        match parents[idx] {
            Some(parent) => idx = parent,
            None => return Some(idx),
        }
    }
    None
}
