use super::*;

fn setup(enabled: bool) -> Workspace<TestWindow> {
    let mut options = Options::default();
    options.layout.floating_windows_follow_focus = enabled;
    options.animations.off = true;
    let mut clock = Clock::with_time(Duration::ZERO);
    clock.set_complete_instantly(true);
    let mut ws = Workspace::new_no_outputs(clock, Rc::new(options));
    ws.set_view_size(
        smithay::output::Scale::Integer(1),
        Transform::Normal,
        (1280., 720.).into(),
        Rectangle::from_size((1280., 720.).into()),
    );
    for (id, floating) in [(1, false), (2, true)] {
        let tile = ws.make_tile(TestWindow::new(TestWindowParams::new(id)));
        ws.add_tile(
            tile,
            WorkspaceAddWindowTarget::Auto,
            ActivateWindow::Yes,
            ColumnWidth::Fixed(800.),
            false,
            floating,
            None,
        );
        settle(&mut ws);
    }
    ws.move_floating_window(
        Some(&2),
        PositionChange::SetFixed(100.),
        PositionChange::SetFixed(100.),
        false,
    );
    settle(&mut ws);
    ws
}

fn settle(ws: &mut Workspace<TestWindow>) {
    ws.refresh(true, true);
    let windows: Vec<_> = ws.windows().cloned().collect();
    for window in windows {
        if window.communicate() {
            ws.update_window(window.id(), None);
        }
    }
    ws.advance_animations();
    ws.verify_invariants(None);
}

fn floating_geometry(ws: &Workspace<TestWindow>) -> Rectangle<f64, Logical> {
    let (tile, pos) = ws.floating().tiles_with_offsets().next().unwrap();
    Rectangle::new(pos, tile.tile_size())
}
fn overlap(ws: &Workspace<TestWindow>) -> Point<f64, Logical> {
    floating_geometry(ws).loc + Point::from((40., 40.))
}
fn hit(ws: &Workspace<TestWindow>, p: Point<f64, Logical>) -> usize {
    *ws.window_under(p).unwrap().0.id()
}
fn visual_order(ws: &Workspace<TestWindow>) -> Vec<usize> {
    ws.tiles_with_render_positions()
        .map(|(t, _, _)| *t.window().id())
        .collect()
}

#[test]
fn floating_stacking_focus_lowers_and_restores_without_minimizing() {
    let mut ws = setup(true);
    let geometry = floating_geometry(&ws);
    let p = overlap(&ws);
    assert!(ws.floating_window_above_scrolling(&2));
    assert_eq!(hit(&ws, p), 2);
    assert_eq!(visual_order(&ws), vec![2, 1]);
    assert!(ws.activate_window(&1));
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&2));
    assert_eq!(hit(&ws, p), 1);
    assert_eq!(visual_order(&ws), vec![1, 2]);
    assert!(ws.is_floating_visible());
    assert_eq!(ws.windows().count(), 2);
    assert_eq!(floating_geometry(&ws), geometry);
    assert!(ws.activate_window(&2));
    settle(&mut ws);
    assert_eq!(hit(&ws, p), 2);
    assert_eq!(visual_order(&ws), vec![2, 1]);
    assert_eq!(floating_geometry(&ws), geometry);
}

#[test]
fn floating_stacking_default_keeps_existing_always_above_behavior() {
    let mut ws = setup(false);
    let p = overlap(&ws);
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(ws.floating_window_above_scrolling(&2));
    assert_eq!(hit(&ws, p), 2);
    assert_eq!(visual_order(&ws), vec![2, 1]);
}

#[test]
fn floating_stacking_explicit_raise_and_workspaces_are_independent() {
    let mut first = setup(true);
    let second = setup(true);
    first.activate_window(&1);
    assert!(!first.floating_window_above_scrolling(&2));
    assert!(second.floating_window_above_scrolling(&2));
    first.activate_window(&2);
    first.activate_window_without_raising(&1);
    assert!(first.floating_window_above_scrolling(&2));
    assert_eq!(hit(&first, overlap(&first)), 2);
}

#[test]
fn floating_stacking_config_reload_changes_order_without_changing_geometry() {
    let mut ws = setup(false);
    ws.activate_window(&1);
    settle(&mut ws);
    let geometry = floating_geometry(&ws);
    let p = overlap(&ws);
    let config =
        niri_config::Config::parse_mem("layout { floating-windows-follow-focus; }").unwrap();
    assert!(config.layout.floating_windows_follow_focus);
    let mut options = Options::default();
    options.animations.off = true;
    options.layout = config.layout;
    ws.update_config(Rc::new(options));
    settle(&mut ws);
    assert_eq!(hit(&ws, p), 1);
    assert_eq!(visual_order(&ws), vec![1, 2]);
    assert_eq!(floating_geometry(&ws), geometry);
}

fn add_float(ws: &mut Workspace<TestWindow>, id: usize, x: f64, activate: ActivateWindow) {
    let tile = ws.make_tile(TestWindow::new(TestWindowParams::new(id)));
    ws.add_tile(
        tile,
        WorkspaceAddWindowTarget::Auto,
        activate,
        ColumnWidth::Fixed(100.),
        false,
        true,
        None,
    );
    ws.move_floating_window(
        Some(&id),
        PositionChange::SetFixed(x),
        PositionChange::SetFixed(100.),
        false,
    );
    settle(ws);
}

#[test]
fn floating_stacking_new_float_does_not_resurrect_lowered_windows() {
    let mut ws = setup(true);
    let old_geometry = floating_geometry(&ws);
    ws.activate_window(&1);
    add_float(&mut ws, 3, 400., ActivateWindow::Yes);
    assert_eq!(visual_order(&ws), vec![3, 1, 2]);
    assert!(!ws.floating_window_above_scrolling(&2));
    assert!(ws.floating_window_above_scrolling(&3));
    assert_eq!(hit(&ws, old_geometry.loc + Point::from((40., 40.))), 1);
    assert_eq!(hit(&ws, Point::from((440., 140.))), 3);
    ws.activate_window(&1);
    add_float(&mut ws, 4, 600., ActivateWindow::Yes);
    assert_eq!(visual_order(&ws), vec![4, 1, 3, 2]);
    ws.remove_tile(&4, crate::utils::transaction::Transaction::new());
    settle(&mut ws);
    assert!(!ws.floating_is_active());
    assert_eq!(visual_order(&ws), vec![1, 3, 2]);
    assert_eq!(hit(&ws, Point::from((440., 140.))), 1);
}

#[test]
fn floating_stacking_reactivates_only_the_requested_lowered_window() {
    let mut ws = setup(true);
    add_float(&mut ws, 3, 400., ActivateWindow::Yes);
    ws.activate_window(&1);
    ws.activate_window(&2);
    settle(&mut ws);
    assert_eq!(visual_order(&ws), vec![2, 1, 3]);
    assert_eq!(hit(&ws, Point::from((140., 140.))), 2);
    assert_eq!(hit(&ws, Point::from((440., 140.))), 1);
    ws.focus_tiling();
    ws.focus_floating();
    assert_eq!(visual_order(&ws), vec![2, 1, 3]);
}

#[test]
fn floating_stacking_unfocused_popup_is_independent_of_older_floats() {
    let mut ws = setup(true);
    ws.activate_window(&1);
    add_float(&mut ws, 3, 400., ActivateWindow::No);
    add_float(&mut ws, 4, 600., ActivateWindow::No);
    assert!(!ws.floating_is_active());
    assert_eq!(visual_order(&ws), vec![4, 3, 1, 2]);
    ws.remove_tile(&3, crate::utils::transaction::Transaction::new());
    settle(&mut ws);
    assert_eq!(visual_order(&ws), vec![4, 1, 2]);
    assert_eq!(hit(&ws, Point::from((640., 140.))), 4);
}

#[test]
fn floating_stacking_parent_activation_raises_its_dialog_but_not_unrelated_float() {
    let mut ws = setup(true);
    add_float(&mut ws, 3, 400., ActivateWindow::Yes);
    let mut params = TestWindowParams::new(4);
    params.parent_id = Some(2);
    let tile = ws.make_tile(TestWindow::new(params));
    ws.add_tile(
        tile,
        WorkspaceAddWindowTarget::NextTo(&2),
        ActivateWindow::Yes,
        ColumnWidth::Fixed(100.),
        false,
        true,
        None,
    );
    ws.activate_window(&1);
    ws.activate_window(&2);
    settle(&mut ws);
    assert!(ws.floating_window_above_scrolling(&2));
    assert!(ws.floating_window_above_scrolling(&4));
    assert!(!ws.floating_window_above_scrolling(&3));
    assert_eq!(visual_order(&ws), vec![4, 2, 1, 3]);
}
