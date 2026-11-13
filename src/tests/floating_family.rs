//! Exercise real xdg_toplevel parent/app-id requests on the isolated headless test compositor.

use niri_config::Config;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Proxy as _;
use wayland_server::Resource as _;

use super::client::ClientId;
use super::Fixture;
use smithay::desktop::Window;

fn map_window(
    f: &mut Fixture,
    client: ClientId,
    app_id: &str,
    parent: Option<&WlSurface>,
) -> (WlSurface, Window) {
    let parent = parent.map(|surface| f.client(client).window(surface).xdg_toplevel.clone());
    let window = f.client(client).create_window();
    let surface = window.surface.clone();
    window.xdg_toplevel.set_app_id(app_id.to_owned());
    window.set_parent(parent.as_ref());
    window.commit();
    f.double_roundtrip(client);
    let window = f.client(client).window(&surface);
    window.attach_new_buffer();
    window.set_size(300, 200);
    window.ack_last_and_commit();
    f.double_roundtrip(client);
    let id = f
        .niri()
        .layout
        .windows()
        .find(|(_, window)| {
            window.toplevel().wl_surface().id().protocol_id() == surface.id().protocol_id()
        })
        .unwrap()
        .1
        .window
        .clone();
    (surface, id)
}

fn above(f: &mut Fixture, id: &Window) -> bool {
    f.niri()
        .layout
        .workspaces()
        .find(|(_, _, ws)| ws.has_window(id))
        .unwrap()
        .2
        .floating_window_above_scrolling(id)
}

#[test]
fn floating_stacking_wayland_parent_and_app_id_families() {
    let config = Config::parse_mem(
        "layout { floating-windows-follow-focus; floating-windows-follow-parent; }",
    )
    .unwrap();
    let mut f = Fixture::with_config(config);
    f.add_output(1, (1920, 1080));
    let client = f.add_client();
    let (owner_surface, owner) = map_window(&mut f, client, "editor", None);
    let (dialog_surface, dialog) =
        map_window(&mut f, client, "dialog-helper", Some(&owner_surface));
    let (_, other) = map_window(&mut f, client, "browser", None);
    f.niri().layout.activate_window(&other);
    f.double_roundtrip(client);
    assert!(!above(&mut f, &dialog));
    f.niri().layout.activate_window(&owner);
    f.double_roundtrip(client);
    assert!(above(&mut f, &dialog));
    assert_eq!(f.niri().layout.focus().unwrap().window, owner);

    let (_, unparented) = map_window(&mut f, client, "editor", None);
    f.niri().layout.set_window_floating(Some(&unparented), true);
    f.niri().layout.activate_window(&other);
    f.double_roundtrip(client);
    assert!(!above(&mut f, &unparented));
    f.niri().layout.activate_window(&owner);
    f.double_roundtrip(client);
    assert!(above(&mut f, &unparented));
    assert!(above(&mut f, &dialog));
    assert_eq!(f.niri().layout.focus().unwrap().window, owner);

    // A second document makes app-id fallback ambiguous, but not explicit parentage.
    let (second_surface, second_document) = map_window(&mut f, client, "editor", None);
    f.niri().layout.activate_window(&second_document);
    f.double_roundtrip(client);
    assert!(!above(&mut f, &unparented));
    assert!(!above(&mut f, &dialog));
    f.niri().layout.activate_window(&owner);
    f.double_roundtrip(client);
    assert!(above(&mut f, &dialog));
    assert!(!above(&mut f, &unparented));

    // Reparenting after mapping updates layers even if no title/window rule changed.
    let new_parent = f
        .client(client)
        .window(&second_surface)
        .xdg_toplevel
        .clone();
    f.client(client)
        .window(&dialog_surface)
        .set_parent(Some(&new_parent));
    f.double_roundtrip(client);
    assert!(!above(&mut f, &dialog));
    f.niri().layout.activate_window(&second_document);
    f.double_roundtrip(client);
    assert!(above(&mut f, &dialog));
}
