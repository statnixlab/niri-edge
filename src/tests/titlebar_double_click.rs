use niri_config::Config;
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_toplevel;
use smithay::reexports::wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;
use wayland_client::protocol::{wl_pointer, wl_surface::WlSurface};
use wayland_client::Proxy as _;
use wayland_server::Resource as _;

use super::client::{ClientId, Configure};
use super::Fixture;

fn setup(extra: &str) -> (Fixture, ClientId, WlSurface, ZwlrVirtualPointerV1) {
    let config = Config::parse_mem(&format!(
        "layout {{ titlebar-double-click-to-column; gaps 16; struts {{ top 40; bottom 30; }}; {extra} }}"
    ))
    .unwrap();
    let mut f = Fixture::with_config(config);
    f.add_output(1, (1920, 1080));
    let id = f.add_client();
    let window = f.client(id).create_window();
    let surface = window.surface.clone();
    window.commit();
    f.roundtrip(id);
    let window = f.client(id).window(&surface);
    window.attach_new_buffer();
    window.set_size(100, 100);
    window.ack_last_and_commit();
    f.double_roundtrip(id);
    let client = f.client(id);
    let pointer = client
        .state
        .virtual_pointer_manager
        .as_ref()
        .unwrap()
        .create_virtual_pointer(None, &client.qh, ());
    (f, id, surface, pointer)
}

fn configure(f: &mut Fixture, id: ClientId, surface: &WlSurface) -> Configure {
    f.client(id)
        .window(surface)
        .configures_received
        .last()
        .unwrap()
        .1
        .clone()
}

fn ack(f: &mut Fixture, id: ClientId, surface: &WlSurface) {
    let size = configure(f, id, surface).size;
    let window = f.client(id).window(surface);
    window.set_size(size.0.try_into().unwrap(), size.1.try_into().unwrap());
    window.ack_last_and_commit();
    f.double_roundtrip(id);
}

fn click(f: &mut Fixture, id: ClientId, surface: &WlSurface, pointer: &ZwlrVirtualPointerV1) {
    f.niri_complete_animations();
    let server_surface = f
        .niri()
        .layout
        .windows()
        .find(|(_, w)| w.toplevel().wl_surface().id().protocol_id() == surface.id().protocol_id())
        .unwrap()
        .1
        .toplevel()
        .wl_surface()
        .clone();
    let position = f
        .niri()
        .layout
        .window_render_location(&server_surface)
        .unwrap();
    pointer.motion_absolute(
        0,
        (position.x + 20.) as u32,
        (position.y + 20.) as u32,
        1920,
        1080,
    );
    pointer.frame();
    f.double_roundtrip(id);
    assert_eq!(
        f.niri().seat.get_pointer().unwrap().current_focus(),
        Some(server_surface)
    );
    pointer.button(0, 0x110, wl_pointer::ButtonState::Pressed);
    f.double_roundtrip(id);
    pointer.button(0, 0x110, wl_pointer::ButtonState::Released);
    f.double_roundtrip(id);
}

#[test]
fn titlebar_double_click_matches_column_shortcut_and_restores() {
    let (mut f, id, surface, pointer) = setup("");
    let original = configure(&mut f, id, &surface).size;
    f.niri().layout.toggle_full_width();
    f.double_roundtrip(id);
    let column = configure(&mut f, id, &surface).size;
    f.niri().layout.toggle_full_width();
    f.double_roundtrip(id);
    for expected in [column, original, column, original] {
        click(&mut f, id, &surface, &pointer);
        click(&mut f, id, &surface, &pointer);
        f.client(id).window(&surface).set_maximized();
        f.double_roundtrip(id);
        let result = configure(&mut f, id, &surface);
        assert_eq!(result.size, expected);
        assert!(!result.states.contains(&xdg_toplevel::State::Maximized));
        ack(&mut f, id, &surface);
    }
}

#[test]
fn titlebar_single_button_clicks_maximize_to_edges_and_restore() {
    // The new option takes precedence over the old blanket override.
    let (mut f, id, surface, pointer) = setup("client-maximize-to-column;");
    let original = configure(&mut f, id, &surface).size;
    for _ in 0..2 {
        click(&mut f, id, &surface, &pointer);
        f.client(id).window(&surface).set_maximized();
        f.double_roundtrip(id);
        let result = configure(&mut f, id, &surface);
        assert_eq!(result.size, (1920, 1080));
        assert!(result.states.contains(&xdg_toplevel::State::Maximized));
        ack(&mut f, id, &surface);
        click(&mut f, id, &surface, &pointer);
        f.client(id).window(&surface).unset_maximized();
        f.double_roundtrip(id);
        assert_eq!(configure(&mut f, id, &surface).size, original);
        ack(&mut f, id, &surface);
    }
}

#[test]
fn titlebar_programmatic_maximize_stays_native() {
    let (mut f, id, surface, _) = setup("");
    for _ in 0..2 {
        f.client(id).window(&surface).set_maximized();
        f.double_roundtrip(id);
        let result = configure(&mut f, id, &surface);
        assert_eq!(result.size, (1920, 1080));
        assert!(result.states.contains(&xdg_toplevel::State::Maximized));
        ack(&mut f, id, &surface);
    }
}

#[test]
fn titlebar_double_click_survives_client_move_grab() {
    let (mut f, id, surface, pointer) = setup("");
    f.niri().layout.toggle_full_width();
    f.double_roundtrip(id);
    let column = configure(&mut f, id, &surface).size;
    f.niri().layout.toggle_full_width();
    f.double_roundtrip(id);

    // Establish focus, then begin a fresh pair. GTK-style titlebars request a
    // move on the first press, even if the pointer never actually moves.
    click(&mut f, id, &surface, &pointer);
    f.niri().titlebar_clicks.clear();
    pointer.button(0, 0x110, wl_pointer::ButtonState::Pressed);
    f.double_roundtrip(id);
    let serial = f
        .niri()
        .seat
        .get_pointer()
        .unwrap()
        .with_grab(|serial, _| serial)
        .unwrap();
    let seat = f.client(id).state.seats.keys().next().unwrap().clone();
    f.client(id)
        .window(&surface)
        .xdg_toplevel
        ._move(&seat, serial.into());
    f.double_roundtrip(id);
    pointer.button(0, 0x110, wl_pointer::ButtonState::Released);
    f.double_roundtrip(id);
    click(&mut f, id, &surface, &pointer);
    f.client(id).window(&surface).set_maximized();
    f.double_roundtrip(id);
    let result = configure(&mut f, id, &surface);
    assert_eq!(result.size, column);
    assert!(!result.states.contains(&xdg_toplevel::State::Maximized));
}

#[test]
fn titlebar_double_click_on_maximized_window_matches_shortcut() {
    let (mut f, id, surface, pointer) = setup("");
    f.client(id).window(&surface).set_maximized();
    f.double_roundtrip(id);
    ack(&mut f, id, &surface);
    f.niri().layout.toggle_full_width();
    f.double_roundtrip(id);
    let shortcut = configure(&mut f, id, &surface).size;
    ack(&mut f, id, &surface);
    f.client(id).window(&surface).set_maximized();
    f.double_roundtrip(id);
    ack(&mut f, id, &surface);
    click(&mut f, id, &surface, &pointer);
    click(&mut f, id, &surface, &pointer);
    f.client(id).window(&surface).unset_maximized();
    f.double_roundtrip(id);
    assert_eq!(configure(&mut f, id, &surface).size, shortcut);
}

#[test]
fn titlebar_floating_window_keeps_native_maximize() {
    let (mut f, id, surface, pointer) = setup("");
    f.niri().layout.toggle_window_floating(None);
    f.double_roundtrip(id);
    f.client(id).window(&surface).set_size(640, 480);
    f.client(id).window(&surface).ack_last_and_commit();
    f.double_roundtrip(id);
    click(&mut f, id, &surface, &pointer);
    click(&mut f, id, &surface, &pointer);
    f.client(id).window(&surface).set_maximized();
    f.double_roundtrip(id);
    assert_eq!(configure(&mut f, id, &surface).size, (1920, 1080));
    ack(&mut f, id, &surface);
    click(&mut f, id, &surface, &pointer);
    f.client(id).window(&surface).unset_maximized();
    f.double_roundtrip(id);
    assert_eq!(configure(&mut f, id, &surface).size, (640, 480));
}
