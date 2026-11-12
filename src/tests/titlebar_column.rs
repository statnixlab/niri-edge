use niri_config::Config;
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_toplevel;
use wayland_client::protocol::wl_surface::WlSurface;

use super::client::{ClientId, Configure};
use super::Fixture;

fn open_window(f: &mut Fixture, id: ClientId) -> WlSurface {
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.roundtrip(id);
    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(100, 100);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
    surface
}

fn last_configure(f: &mut Fixture, id: ClientId, surface: &WlSurface) -> Configure {
    f.client(id)
        .window(surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone()
}

fn set_up() -> (Fixture, ClientId, WlSurface) {
    let config = Config::parse_mem(
        "layout { client-maximize-to-column; gaps 16; struts { top 40; bottom 30; }; }",
    )
    .unwrap();
    let mut f = Fixture::with_config(config);
    f.add_output(1, (1920, 1080));
    let id = f.add_client();
    let surface = open_window(&mut f, id);
    (f, id, surface)
}

#[test]
fn titlebar_column_matches_shortcut_and_restores() {
    let (mut f, id, surface) = set_up();
    let original = last_configure(&mut f, id, &surface);
    f.niri().layout.toggle_full_width();
    f.double_roundtrip(id);
    let shortcut = last_configure(&mut f, id, &surface);
    assert!(shortcut.size.0 > original.size.0);
    assert!(shortcut.size.0 < 1920 && shortcut.size.1 < 1010);
    f.niri().layout.toggle_full_width();
    f.double_roundtrip(id);
    for _ in 0..2 {
        f.client(id).window(&surface).set_maximized();
        f.double_roundtrip(id);
        let clicked = last_configure(&mut f, id, &surface);
        assert_eq!(clicked.size, shortcut.size);
        assert!(!clicked.states.contains(&xdg_toplevel::State::Maximized));
        f.client(id).window(&surface).ack_last_and_commit();
        f.double_roundtrip(id);
        f.client(id).window(&surface).set_maximized();
        f.double_roundtrip(id);
        assert_eq!(last_configure(&mut f, id, &surface).size, original.size);
        f.client(id).window(&surface).ack_last_and_commit();
        f.double_roundtrip(id);
    }
}

#[test]
fn titlebar_column_targets_requesting_window() {
    let (mut f, id, first) = set_up();
    let second = open_window(&mut f, id);
    let first_before = last_configure(&mut f, id, &first).size;
    let second_before = last_configure(&mut f, id, &second).size;
    f.client(id).window(&first).set_maximized();
    f.double_roundtrip(id);
    assert!(last_configure(&mut f, id, &first).size.0 > first_before.0);
    assert_eq!(last_configure(&mut f, id, &second).size, second_before);
}

#[test]
fn titlebar_column_maximizes_floating_and_restores() {
    let (mut f, id, surface) = set_up();
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    f.client(id).window(&surface).set_size(640, 480);
    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);
    let original_pos = f
        .niri()
        .layout
        .active_workspace()
        .unwrap()
        .floating()
        .tiles_with_offsets()
        .next()
        .unwrap()
        .1;
    f.niri().layout.toggle_full_width();
    f.double_roundtrip(id);
    for _ in 0..2 {
        f.client(id).window(&surface).set_maximized();
        f.double_roundtrip(id);
        let clicked = last_configure(&mut f, id, &surface);
        assert_eq!(clicked.size, (1920, 1080));
        assert!(clicked.states.contains(&xdg_toplevel::State::Maximized));
        f.client(id).window(&surface).set_size(1920, 1080);
        f.client(id).window(&surface).ack_last_and_commit();
        f.double_roundtrip(id);

        // A redundant protocol request must not toggle the maximized column.
        f.client(id).window(&surface).set_maximized();
        f.double_roundtrip(id);
        let repeated = last_configure(&mut f, id, &surface);
        assert_eq!(repeated.size, clicked.size);
        assert!(repeated.states.contains(&xdg_toplevel::State::Maximized));

        f.client(id).window(&surface).unset_maximized();
        f.double_roundtrip(id);
        let restored = last_configure(&mut f, id, &surface);
        assert_eq!(restored.size, (640, 480));
        assert!(!restored.states.contains(&xdg_toplevel::State::Maximized));
        f.client(id).window(&surface).set_size(640, 480);
        f.client(id).window(&surface).ack_last_and_commit();
        f.double_roundtrip(id);
        let restored_pos = f
            .niri()
            .layout
            .active_workspace()
            .unwrap()
            .floating()
            .tiles_with_offsets()
            .next()
            .unwrap()
            .1;
        assert_eq!(restored_pos, original_pos);
    }
}
