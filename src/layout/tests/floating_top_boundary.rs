use super::*;

fn setup(enabled: bool) -> Layout<TestWindow> {
    let mut options = Options::default();
    options.layout.floating_windows_match_tiling_top = enabled;
    options.layout.gaps = 3.;
    options.layout.struts.top = FloatOrInt(-3.);
    options.layout.border.off = true;
    options.animations.off = true;
    let mut clock = Clock::with_time(Duration::ZERO);
    clock.set_complete_instantly(true);
    let mut layout = Layout::with_options(clock, options);
    Op::AddOutput(1).apply(&mut layout);
    let mut params = TestWindowParams::new(1);
    params.is_floating = true;
    Op::AddWindow { params }.apply(&mut layout);
    layout
}

fn pos(layout: &Layout<TestWindow>) -> Point<f64, Logical> {
    layout
        .workspaces()
        .map(|(_, _, ws)| ws)
        .find(|ws| ws.has_window(&1))
        .unwrap()
        .floating()
        .tiles_with_offsets()
        .next()
        .unwrap()
        .1
}

fn move_to(layout: &mut Layout<TestWindow>, x: f64, y: f64) {
    layout.move_floating_window(
        Some(&1),
        PositionChange::SetFixed(x),
        PositionChange::SetFixed(y),
        false,
    );
    layout.advance_animations();
}

#[test]
fn floating_top_boundary_config_and_command_moves() {
    let config = Config::parse_mem(
        "layout { floating-windows-match-tiling-top; gaps 3; struts { top -3; }; }",
    )
    .unwrap();
    assert!(config.layout.floating_windows_match_tiling_top);
    let mut layout = setup(config.layout.floating_windows_match_tiling_top);
    move_to(&mut layout, 100., -1000.);
    assert_eq!(pos(&layout), Point::from((100., 0.)));
    move_to(&mut layout, -40., 200.);
    assert_eq!(pos(&layout), Point::from((-40., 200.)));
    move_to(&mut layout, 100., 1000.);
    assert!(pos(&layout).y > 600.);
}

#[test]
fn floating_top_boundary_drag_is_clamped_before_release_and_can_move_back_down() {
    let mut layout = setup(true);
    move_to(&mut layout, 100., 100.);
    let output = layout.outputs().next().unwrap().clone();
    assert!(layout.interactive_move_begin(1, &output, (120., 120.).into()));
    assert!(layout.interactive_move_update(
        &1,
        (0., -120.).into(),
        output.clone(),
        (120., 0.).into()
    ));
    let moving = layout.interactive_move.as_ref().unwrap().moving().unwrap();
    assert!(moving.tile_render_location(1.).y >= 0.);
    layout.advance_animations();
    let moving = layout.interactive_move.as_ref().unwrap().moving().unwrap();
    assert_eq!(moving.tile_render_location(1.).y, 0.);
    assert!(layout.interactive_move_update(
        &1,
        (0., 200.).into(),
        output.clone(),
        (120., 200.).into()
    ));
    let moving = layout.interactive_move.as_ref().unwrap().moving().unwrap();
    assert_eq!(moving.tile_render_location(1.).y, 180.);
    layout.interactive_move_update(&1, (0., -200.).into(), output, (120., 0.).into());
    layout.interactive_move_end(&1);
    assert_eq!(pos(&layout).y, 0.);
}

#[test]
fn floating_top_boundary_reload_and_panel_changes_follow_tiling_top() {
    let mut layout = setup(false);
    move_to(&mut layout, 100., -1000.);
    assert!(pos(&layout).y < 0.);
    layout.update_config(
        &Config::parse_mem(
            "layout { floating-windows-match-tiling-top; gaps 3; struts { top -3; }; }",
        )
        .unwrap(),
    );
    assert_eq!(pos(&layout).y, 0.);
    let ws = layout
        .workspaces_mut()
        .find(|ws| ws.has_window(&1))
        .unwrap();
    // A visible panel reserves 30 logical pixels; hiding it removes that reservation.
    for top in [30., 55., 12., 0.] {
        ws.set_view_size(
            smithay::output::Scale::Integer(1),
            Transform::Normal,
            (1280., 720.).into(),
            Rectangle::new((0., top).into(), (1280., 720. - top).into()),
        );
        ws.move_floating_window(
            Some(&1),
            PositionChange::SetFixed(100.),
            PositionChange::SetFixed(-1000.),
            false,
        );
        assert_eq!(ws.floating().tiles_with_offsets().next().unwrap().1.y, top);
        ws.verify_invariants(None);
    }
}

#[test]
fn floating_top_boundary_disabled_preserves_upstream_dragging() {
    let mut layout = setup(false);
    move_to(&mut layout, 100., 100.);
    let output = layout.outputs().next().unwrap().clone();
    layout.interactive_move_begin(1, &output, (120., 120.).into());
    layout.interactive_move_update(&1, (0., -120.).into(), output, (120., 0.).into());
    layout.advance_animations();
    let moving = layout.interactive_move.as_ref().unwrap().moving().unwrap();
    assert_eq!(moving.tile_render_location(1.).y, -20.);
    layout.interactive_move_end(&1);
    assert_eq!(pos(&layout).y, -20.);
}

#[test]
fn floating_top_boundary_does_not_constrain_native_maximize_or_fullscreen() {
    let mut layout = setup(true);
    move_to(&mut layout, 100., 30.);
    let ws = layout
        .workspaces_mut()
        .find(|ws| ws.has_window(&1))
        .unwrap();
    ws.set_maximized(&1, true);
    assert!(!ws.is_floating(&1));
    ws.set_maximized(&1, false);
    assert!(ws.is_floating(&1));
    assert!(ws.floating().tiles_with_offsets().next().unwrap().1.y >= 0.);
    ws.set_fullscreen(&1, true);
    assert!(!ws.is_floating(&1));
    ws.set_fullscreen(&1, false);
    assert!(ws.is_floating(&1));
    assert!(ws.floating().tiles_with_offsets().next().unwrap().1.y >= 0.);
}

#[test]
fn floating_top_boundary_matches_tiled_top_with_struts_gaps_and_scale() {
    for scale in [1., 1.25, 2.] {
        for (reserved, strut, gap) in [(30., -3., 3.), (54., 4.4, 7.2), (0., 0., 5.)] {
            let mut layout = setup(true);
            Op::AddWindow {
                params: TestWindowParams::new(2),
            }
            .apply(&mut layout);
            let ws = layout
                .workspaces_mut()
                .find(|ws| ws.has_window(&1))
                .unwrap();
            let mut options = Options::default();
            options.layout.floating_windows_match_tiling_top = true;
            options.layout.struts.top = FloatOrInt(strut);
            options.layout.gaps = gap;
            options.layout.border.off = true;
            options.animations.off = true;
            ws.update_config(Rc::new(options));
            ws.set_view_size(
                smithay::output::Scale::Fractional(scale),
                Transform::Normal,
                (1280., 720.).into(),
                Rectangle::new((0., reserved).into(), (1280., 720. - reserved).into()),
            );
            ws.move_floating_window(
                Some(&1),
                PositionChange::SetFixed(100.),
                PositionChange::SetFixed(-1000.),
                false,
            );
            ws.advance_animations();
            let floating_y = ws
                .tiles_with_render_positions()
                .find(|(tile, _, _)| *tile.window().id() == 1)
                .unwrap()
                .1
                .y;
            let tiled_y = ws
                .tiles_with_render_positions()
                .find(|(tile, _, _)| *tile.window().id() == 2)
                .unwrap()
                .1
                .y;
            assert_eq!(
                floating_y, tiled_y,
                "scale={scale}, reserved={reserved}, strut={strut}, gap={gap}"
            );
        }
    }
}
