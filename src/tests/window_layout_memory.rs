//! Real Wayland clients and isolated persistence; never read/write the user's state file.

use niri_config::Config;
use smithay::desktop::Window;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Proxy as _;
use wayland_server::Resource as _;

use super::client::ClientId;
use super::Fixture;
use crate::window_layout_memory::WindowLayoutMemory;

fn setup(path: Option<std::path::PathBuf>) -> Fixture {
    let config = Config::parse_mem(
        r#"
        layout { remember-window-layout; }
        window-rule { match title="^utility$"; remember-window-layout false; open-floating true; }
    "#,
    )
    .unwrap();
    let mut fixture = Fixture::with_config(config);
    fixture.niri().window_layout_memory = WindowLayoutMemory::new(path);
    fixture.add_output(1, (1280, 720));
    fixture
}

fn window(
    f: &mut Fixture,
    client: ClientId,
    title: &str,
    parent: Option<&WlSurface>,
    fixed: bool,
) -> (WlSurface, Window) {
    let parent = parent.map(|surface| f.client(client).window(surface).xdg_toplevel.clone());
    let w = f.client(client).create_window();
    let surface = w.surface.clone();
    w.xdg_toplevel.set_app_id("test.editor".into());
    w.set_title(title);
    w.set_parent(parent.as_ref());
    if fixed {
        w.xdg_toplevel.set_min_size(300, 200);
        w.xdg_toplevel.set_max_size(300, 200);
    }
    w.commit();
    f.double_roundtrip(client);
    let w = f.client(client).window(&surface);
    w.attach_new_buffer();
    w.set_size(300, 200);
    w.ack_last_and_commit();
    f.double_roundtrip(client);
    let mapped = f
        .niri()
        .layout
        .windows()
        .find(|(_, w)| w.toplevel().wl_surface().id().protocol_id() == surface.id().protocol_id())
        .unwrap()
        .1
        .window
        .clone();
    (surface, mapped)
}

fn floating(f: &mut Fixture, window: &Window) -> bool {
    f.niri()
        .layout
        .workspaces()
        .find(|(_, _, ws)| ws.has_window(window))
        .unwrap()
        .2
        .is_floating(window)
}

#[test]
fn window_layout_memory_restores_across_sessions_and_records_immediate_close() {
    let shell = xshell::Shell::new().unwrap();
    let dir = shell.create_temp_dir().unwrap();
    let path = dir.path().join("window-layouts.json");
    let mut f = setup(Some(path.clone()));
    let client = f.add_client();
    let (surface, main) = window(&mut f, client, "main", None, false);
    assert!(!floating(&mut f, &main));
    f.niri().layout.set_window_floating(Some(&main), true);
    // Destroy before the next normal refresh: the close path must record the final layout too.
    f.client(client).window(&surface).xdg_toplevel.destroy();
    f.double_roundtrip(client);
    assert_eq!(
        f.niri().window_layout_memory.preference("test.editor"),
        Some(true)
    );
    drop(f);
    let mut f = setup(Some(path.clone()));
    let client = f.add_client();
    let (_, main) = window(&mut f, client, "reopened", None, false);
    assert!(floating(&mut f, &main));
    f.niri().layout.set_window_floating(Some(&main), false);
    f.double_roundtrip(client);
    drop(f);
    let mut f = setup(Some(path));
    let client = f.add_client();
    let (_, main) = window(&mut f, client, "reopened-again", None, false);
    assert!(!floating(&mut f, &main));
}

#[test]
fn window_layout_memory_does_not_reclassify_or_learn_dialogs_and_utilities() {
    let mut f = setup(None);
    let client = f.add_client();
    let (surface, main) = window(&mut f, client, "main", None, false);
    let (_, dialog) = window(&mut f, client, "dialog", Some(&surface), false);
    let (_, fixed) = window(&mut f, client, "fixed", None, true);
    let (_, utility) = window(&mut f, client, "utility", None, false);
    assert!(!floating(&mut f, &main));
    for w in [&dialog, &fixed, &utility] {
        assert!(floating(&mut f, w));
    }
    assert_eq!(
        f.niri().window_layout_memory.preference("test.editor"),
        Some(false)
    );
    let (_, next) = window(&mut f, client, "second-main", None, false);
    assert!(!floating(&mut f, &next));
}

#[test]
fn window_layout_memory_uses_latest_switch_and_not_fullscreen_or_minimization() {
    let mut f = setup(None);
    let client = f.add_client();
    let (_, first) = window(&mut f, client, "first", None, false);
    f.niri().layout.set_window_floating(Some(&first), true);
    f.double_roundtrip(client);
    let (_, second) = window(&mut f, client, "second", None, false);
    assert!(floating(&mut f, &second));
    f.niri().layout.set_fullscreen(&first, true);
    f.double_roundtrip(client);
    assert_eq!(
        f.niri().window_layout_memory.preference("test.editor"),
        Some(true)
    );
    f.niri().layout.set_fullscreen(&first, false);
    f.double_roundtrip(client);
    f.niri().layout.minimize_window(&first);
    f.double_roundtrip(client);
    assert_eq!(
        f.niri().window_layout_memory.preference("test.editor"),
        Some(true)
    );
    f.niri().layout.set_window_floating(Some(&second), false);
    f.double_roundtrip(client);
    let (_, third) = window(&mut f, client, "third", None, false);
    assert!(!floating(&mut f, &third));
}

#[test]
fn window_layout_memory_overrides_opening_defaults_only_when_enabled() {
    for enabled in [false, true] {
        let prefix = if enabled {
            "layout { remember-window-layout; }"
        } else {
            ""
        };
        let config =
            Config::parse_mem(&format!("{prefix}\nwindow-rule {{ open-floating true; }}")).unwrap();
        let mut f = Fixture::with_config(config);
        f.add_output(1, (1280, 720));
        f.niri()
            .window_layout_memory
            .observe([(999, Some(("test.editor".into(), false)))]);
        let client = f.add_client();
        let (_, main) = window(&mut f, client, "main", None, false);
        assert_eq!(floating(&mut f, &main), !enabled);
    }
}
