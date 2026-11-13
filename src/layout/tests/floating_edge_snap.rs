use super::*;

fn settle(layout: &mut Layout<TestWindow>) {
    for _ in 0..3 {
        layout.refresh(true);
        let windows: Vec<_> = layout.windows().map(|(_, win)| win.clone()).collect();
        for win in windows {
            if win.communicate() {
                layout.update_window(win.id(), None);
            }
        }
        layout.advance_animations();
    }
    layout.verify_invariants();
}

fn setup(enabled: bool, scale: f64, existing: bool, centered: bool) -> Layout<TestWindow> {
    let mut options = Options::default();
    options.layout.floating_windows_edge_snap = enabled;
    options.layout.floating_windows_match_tiling_top = true;
    options.layout.gaps = 4.;
    options.layout.struts.top = FloatOrInt(30.);
    options.layout.border.off = true;
    if centered {
        options.layout.center_focused_column = CenterFocusedColumn::Always;
    }
    options.animations.off = true;
    let mut clock = Clock::with_time(Duration::ZERO);
    clock.set_complete_instantly(true);
    let mut layout = Layout::with_options(clock, options);
    Op::AddScaledOutput {
        id: 1,
        scale,
        layout_config: None,
    }
    .apply(&mut layout);
    if existing {
        for id in [2, 3] {
            Op::AddWindow {
                params: TestWindowParams::new(id),
            }
            .apply(&mut layout);
        }
    }
    let mut params = TestWindowParams::new(1);
    params.is_floating = true;
    Op::AddWindow { params }.apply(&mut layout);
    settle(&mut layout);
    layout
}

fn start_drag(layout: &mut Layout<TestWindow>, x: f64, output: Output) {
    let (mon, _, ws) = layout
        .workspaces()
        .find(|(_, _, ws)| ws.has_window(&1))
        .unwrap();
    let (tile, pos) = ws.floating().tiles_with_offsets().next().unwrap();
    let start = pos + tile.window_loc() + Point::from((10., 10.));
    let origin = mon.unwrap().output().clone();
    assert!(layout.interactive_move_begin(1, &origin, start));
    assert!(layout.interactive_move_update(
        &1,
        (x - start.x, 100. - start.y).into(),
        output,
        (x, 100.).into()
    ));
    layout.advance_animations();
    layout.update_insert_hint(None);
}

fn geometry(layout: &Layout<TestWindow>) -> (bool, Point<f64, Logical>, Size<f64, Logical>) {
    let (_, _, ws) = layout
        .workspaces()
        .find(|(_, _, ws)| ws.has_window(&1))
        .unwrap();
    let (tile, pos, _) = ws
        .tiles_with_render_positions()
        .find(|(tile, _, _)| *tile.window().id() == 1)
        .unwrap();
    (ws.is_floating(&1), pos, tile.tile_size())
}

#[test]
fn floating_edge_snap_preview_then_drop_to_matching_half() {
    for scale in [1., 2.] {
        for existing in [false, true] {
            for centered in [false, true] {
                for side in [EdgeSnapSide::Left, EdgeSnapSide::Right] {
                    let mut layout = setup(true, scale, existing, centered);
                    let output = layout.outputs().next().unwrap().clone();
                    let view = output_size(&output);
                    let x = if side == EdgeSnapSide::Left {
                        0.
                    } else {
                        view.w - 1. / scale
                    };
                    start_drag(&mut layout, x, output.clone());
                    assert!(
                        layout
                            .interactive_move
                            .as_ref()
                            .unwrap()
                            .moving()
                            .unwrap()
                            .is_floating
                    );
                    let mon = layout.monitor_for_output(&output).unwrap();
                    assert_eq!(
                        mon.insert_hint.as_ref().unwrap().position,
                        InsertPosition::EdgeSnap(side)
                    );
                    let preview = mon.workspaces[mon.active_workspace_idx]
                        .insert_hint_area(InsertPosition::EdgeSnap(side))
                        .unwrap();
                    assert_eq!(preview.loc.y, 34.);
                    assert_eq!(preview.size.w, (view.w - 4.) * 0.5 - 4.);
                    layout.interactive_move_end(&1);
                    settle(&mut layout);
                    let (floating, pos, size) = geometry(&layout);
                    assert!(!floating);
                    assert_eq!(size.w, preview.size.w.floor());
                    assert_eq!(size.h, view.h - 38.);
                    assert_eq!(pos.y, 34.);
                    let expected_x = if side == EdgeSnapSide::Left {
                        4.
                    } else {
                        view.w - 4. - size.w
                    };
                    assert!((pos.x - expected_x).abs() < 0.01, "side={side:?} scale={scale} existing={existing} centered={centered}: {pos:?}");
                    assert_eq!(layout.windows().count(), if existing { 3 } else { 1 });
                }
            }
        }
    }
}

#[test]
fn floating_edge_snap_cancel_before_release_and_disabled_option() {
    for enabled in [false, true] {
        let mut layout = setup(enabled, 1., false, false);
        let output = layout.outputs().next().unwrap().clone();
        start_drag(&mut layout, 0., output.clone());
        if enabled {
            layout.interactive_move_update(
                &1,
                (400., 0.).into(),
                output.clone(),
                (400., 100.).into(),
            );
            layout.update_insert_hint(None);
        }
        assert_eq!(
            layout
                .monitor_for_output(&output)
                .unwrap()
                .insert_hint
                .as_ref()
                .unwrap()
                .position,
            InsertPosition::Floating
        );
        layout.interactive_move_end(&1);
        settle(&mut layout);
        assert!(geometry(&layout).0);
    }
}

#[test]
fn floating_edge_snap_uses_destination_output_and_leaves_other_output_intact() {
    let mut layout = setup(true, 1., true, false);
    Op::AddScaledOutput {
        id: 2,
        scale: 2.,
        layout_config: None,
    }
    .apply(&mut layout);
    let output = layout
        .outputs()
        .find(|output| output.name() == "output2")
        .unwrap()
        .clone();
    start_drag(&mut layout, 639.5, output.clone());
    layout.interactive_move_end(&1);
    settle(&mut layout);
    let (floating, pos, size) = geometry(&layout);
    assert!(!floating);
    assert_eq!(size.w, 314.);
    assert_eq!(pos.x, 322.);
    let (mon, _, _) = layout
        .workspaces()
        .find(|(_, _, ws)| ws.has_window(&1))
        .unwrap();
    assert_eq!(mon.unwrap().output(), &output);
    for id in [2, 3] {
        let (mon, _, _) = layout
            .workspaces()
            .find(|(_, _, ws)| ws.has_window(&id))
            .unwrap();
        assert_eq!(mon.unwrap().output().name(), "output1");
    }
}

#[test]
fn floating_edge_snap_does_not_apply_in_overview_or_to_tiled_drags() {
    let mut layout = setup(true, 1., false, false);
    let output = layout.outputs().next().unwrap().clone();
    start_drag(&mut layout, 0., output);
    let moving = layout
        .interactive_move
        .as_mut()
        .unwrap()
        .moving_mut()
        .unwrap();
    assert_eq!(moving.edge_snap(1., true), None);
    assert_eq!(moving.edge_snap(0.5, false), None);
    moving.is_floating = false;
    assert_eq!(moving.edge_snap(1., false), None);
}

#[test]
fn floating_edge_snap_expanded_float_becomes_half_width_and_can_float_again() {
    let mut layout = setup(true, 1., false, false);
    let original_size = geometry(&layout).2;
    layout.toggle_floating_expanded(&1);
    settle(&mut layout);
    let output = layout.outputs().next().unwrap().clone();
    start_drag(&mut layout, 0., output);
    layout.interactive_move_end(&1);
    settle(&mut layout);
    assert_eq!(geometry(&layout).2.w, 634.);
    layout.toggle_window_floating(Some(&1));
    settle(&mut layout);
    assert!(geometry(&layout).0);
    assert_eq!(geometry(&layout).2, original_size);
}
