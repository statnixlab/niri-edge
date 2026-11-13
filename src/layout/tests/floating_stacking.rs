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

fn enable_families(ws: &mut Workspace<TestWindow>) {
    let mut options = Options::clone(&ws.base_options);
    options.layout.floating_windows_follow_parent = true;
    ws.update_config(Rc::new(options));
    settle(ws);
}

fn add_family_window(
    ws: &mut Workspace<TestWindow>,
    id: usize,
    parent: Option<usize>,
    app_id: Option<&str>,
    floating: bool,
    activate: ActivateWindow,
) -> TestWindow {
    let mut params = TestWindowParams::new(id);
    params.parent_id = parent;
    let window = TestWindow::new(params);
    *window.0.app_id.borrow_mut() = app_id.map(str::to_owned);
    ws.add_tile(
        ws.make_tile(window.clone()),
        WorkspaceAddWindowTarget::Auto,
        activate,
        ColumnWidth::Fixed(800.),
        false,
        floating,
        None,
    );
    settle(ws);
    window
}

#[test]
fn floating_stacking_family_tiled_owner_restores_nested_dialogs_and_input_order() {
    let mut ws = setup(true);
    enable_families(&mut ws);
    let dialog = ws.windows().find(|w| *w.id() == 2).unwrap().clone();
    dialog.0.parent_id.set(Some(1));
    let geometry = floating_geometry(&ws);
    let p = overlap(&ws);
    add_family_window(&mut ws, 3, Some(2), None, true, ActivateWindow::Yes);
    add_family_window(&mut ws, 4, None, None, false, ActivateWindow::Yes);
    assert!(!ws.floating_window_above_scrolling(&2));
    assert!(!ws.floating_window_above_scrolling(&3));
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(ws.floating_window_above_scrolling(&2));
    assert!(ws.floating_window_above_scrolling(&3));
    assert_eq!(ws.active_window().unwrap().id(), &1);
    assert_eq!(&visual_order(&ws)[..2], &[3, 2]);
    // Remove the nested dialog so the original overlap point tests the direct child.
    ws.remove_tile(&3, crate::utils::transaction::Transaction::new());
    settle(&mut ws);
    assert_eq!(hit(&ws, p), 2);
    assert_eq!(floating_geometry(&ws), geometry);
}

#[test]
fn floating_stacking_family_floating_owner_and_siblings_switch_together() {
    let mut ws = setup(true);
    enable_families(&mut ws);
    add_family_window(&mut ws, 3, Some(2), None, true, ActivateWindow::Yes);
    add_family_window(&mut ws, 4, Some(2), None, true, ActivateWindow::Yes);
    add_family_window(&mut ws, 5, None, None, false, ActivateWindow::Yes);
    for id in [2, 3, 4] {
        assert!(!ws.floating_window_above_scrolling(&id));
    }
    ws.activate_window(&3);
    settle(&mut ws);
    for id in [2, 3, 4] {
        assert!(ws.floating_window_above_scrolling(&id));
    }
    let order = visual_order(&ws);
    for child in [3, 4] {
        assert!(order.iter().position(|id| *id == child) < order.iter().position(|id| *id == 2));
    }
    assert_eq!(ws.active_window().unwrap().id(), &3);
}

#[test]
fn floating_stacking_family_declared_parent_wins_over_same_app_other_document() {
    let mut ws = setup(true);
    enable_families(&mut ws);
    add_family_window(&mut ws, 3, None, Some("editor"), false, ActivateWindow::Yes);
    add_family_window(
        &mut ws,
        4,
        Some(1),
        Some("editor"),
        true,
        ActivateWindow::No,
    );
    assert!(!ws.floating_window_above_scrolling(&4));
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(ws.floating_window_above_scrolling(&4));
    ws.activate_window(&3);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&4));
}

#[test]
fn floating_stacking_family_unique_app_fallback_and_late_app_id() {
    let mut ws = setup(true);
    enable_families(&mut ws);
    let owner = ws.windows().find(|w| *w.id() == 1).unwrap().clone();
    *owner.0.app_id.borrow_mut() = Some("editor".into());
    let dialog = add_family_window(&mut ws, 3, None, None, true, ActivateWindow::Yes);
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&3));
    *dialog.0.app_id.borrow_mut() = Some("editor".into());
    settle(&mut ws);
    assert!(ws.floating_window_above_scrolling(&3));
    assert!(!ws.floating_window_above_scrolling(&2));
    add_family_window(&mut ws, 4, None, Some("other"), false, ActivateWindow::Yes);
    assert!(!ws.floating_window_above_scrolling(&3));
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(ws.floating_window_above_scrolling(&3));
}

#[test]
fn floating_stacking_family_ambiguous_app_and_missing_declared_parent_are_not_guessed() {
    let mut ws = setup(true);
    enable_families(&mut ws);
    let owner = ws.windows().find(|w| *w.id() == 1).unwrap().clone();
    *owner.0.app_id.borrow_mut() = Some("editor".into());
    add_family_window(&mut ws, 3, None, Some("editor"), false, ActivateWindow::Yes);
    add_family_window(&mut ws, 4, None, Some("editor"), true, ActivateWindow::Yes);
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&4));
    ws.activate_window(&3);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&4));
    ws.remove_tile(&3, crate::utils::transaction::Transaction::new());
    let dialog = ws.windows().find(|w| *w.id() == 4).unwrap().clone();
    dialog.0.parent_id.set(Some(999));
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&4));
}

#[test]
fn floating_stacking_family_keyboard_focus_and_pointer_no_raise() {
    let mut ws = setup(true);
    enable_families(&mut ws);
    add_family_window(&mut ws, 3, Some(1), None, true, ActivateWindow::No);
    add_family_window(&mut ws, 4, None, None, false, ActivateWindow::Yes);
    ws.focus_column_first();
    settle(&mut ws);
    assert_eq!(ws.active_window().unwrap().id(), &1);
    assert!(ws.floating_window_above_scrolling(&3));
    ws.focus_column_last();
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&3));
    ws.activate_window_without_raising(&1);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&3));
    add_family_window(&mut ws, 5, Some(1), None, true, ActivateWindow::No);
    assert!(!ws.floating_window_above_scrolling(&5));
    ws.activate_window_without_raising(&3);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&3));
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(ws.floating_window_above_scrolling(&3));
    assert!(ws.floating_window_above_scrolling(&5));
}

#[test]
fn floating_stacking_family_background_dialog_does_not_cover_other_app() {
    let mut ws = setup(true);
    enable_families(&mut ws);
    add_family_window(&mut ws, 3, None, None, false, ActivateWindow::Yes);
    add_family_window(&mut ws, 4, Some(1), None, true, ActivateWindow::No);
    assert_eq!(ws.active_window().unwrap().id(), &3);
    assert!(!ws.floating_window_above_scrolling(&4));
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(ws.floating_window_above_scrolling(&4));
}

#[test]
fn floating_stacking_family_config_reload_and_reparenting() {
    let mut ws = setup(true);
    let dialog = add_family_window(&mut ws, 3, Some(1), None, true, ActivateWindow::No);
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&3));
    let config = niri_config::Config::parse_mem(
        "layout { floating-windows-follow-focus; floating-windows-follow-parent; }",
    )
    .unwrap();
    assert!(config.layout.floating_windows_follow_parent);
    enable_families(&mut ws);
    assert!(ws.floating_window_above_scrolling(&3));
    add_family_window(&mut ws, 4, None, None, false, ActivateWindow::Yes);
    dialog.0.parent_id.set(Some(4));
    settle(&mut ws);
    assert!(ws.floating_window_above_scrolling(&3));
    ws.activate_window(&1);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&3));
    let mut options = Options::clone(&ws.base_options);
    options.layout.floating_windows_follow_parent = false;
    ws.update_config(Rc::new(options));
    ws.activate_window(&4);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&3));
}

#[test]
fn floating_stacking_family_fullscreen_owner_keeps_its_dialog_accessible() {
    let mut ws = setup(true);
    enable_families(&mut ws);
    let dialog = ws.windows().find(|w| *w.id() == 2).unwrap().clone();
    dialog.0.parent_id.set(Some(1));
    ws.activate_window(&1);
    ws.set_fullscreen(&1, true);
    settle(&mut ws);
    assert!(ws.fullscreen_window_above_top_layer().is_some());
    assert!(ws.is_floating_visible());
    assert_eq!(hit(&ws, overlap(&ws)), 2);
    add_family_window(&mut ws, 3, None, None, false, ActivateWindow::Yes);
    ws.set_fullscreen(&3, true);
    settle(&mut ws);
    assert!(!ws.floating_window_above_scrolling(&2));
    assert!(!ws.is_floating_visible());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn floating_stacking_family_random_layout_operations(
        ops in prop::collection::vec(any::<Op>(), 1..60),
    ) {
        let mut options = Options::default();
        options.layout.floating_windows_follow_focus = true;
        options.layout.floating_windows_follow_parent = true;
        let mut layout = Layout::with_options(Clock::with_time(Duration::ZERO), options);
        for op in ops {
            op.apply(&mut layout);
            layout.refresh(true);
            layout.verify_invariants();
        }
    }
}
