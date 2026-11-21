//! Real Wayland clients and isolated persistence; never read/write the user's state file.

use niri_config::Config;
use niri_ipc::SizeChange;
use smithay::desktop::Window;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Proxy as _;
use wayland_server::Resource as _;

use super::client::ClientId;
use super::Fixture;
use crate::window_size_memory::WindowSizeMemory;

fn setup(path: Option<std::path::PathBuf>) -> Fixture {
    let config = Config::parse_mem(
        r#"
        layout { remember-window-size; }
        window-rule { match title="^float"; open-floating true; }
        window-rule {
            match title="^dialog$"
            open-floating true
            default-column-width { fixed 900; }
            default-window-height { fixed 520; }
            window-memory-group "dialog"
        }
        window-rule { match title="^excluded$"; open-floating true; remember-window-size false; }
    "#,
    )
    .unwrap();
    let mut fixture = Fixture::with_config(config);
    fixture.niri().window_size_memory = WindowSizeMemory::new(path);
    fixture.add_output(1, (1920, 1080));
    fixture
}

/// Opens a window, follows its initial configure and returns it with that configure's size.
fn open(f: &mut Fixture, client: ClientId, title: &str) -> (WlSurface, Window, (i32, i32)) {
    let w = f.client(client).create_window();
    let surface = w.surface.clone();
    w.xdg_toplevel.set_app_id("test.app".into());
    w.set_title(title);
    w.commit();
    f.double_roundtrip(client);
    let w = f.client(client).window(&surface);
    let (_, configure) = w.configures_received.last().unwrap();
    let size = configure.size;
    // A zero dimension leaves the choice to the client.
    let width = if size.0 > 0 { size.0 } else { 300 };
    let height = if size.1 > 0 { size.1 } else { 200 };
    w.attach_new_buffer();
    w.set_size(width as u16, height as u16);
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
    (surface, mapped, size)
}

fn resize_floating(f: &mut Fixture, client: ClientId, surface: &WlSurface, w: u16, h: u16) {
    let window = f.client(client).window(surface);
    window.set_size(w, h);
    window.commit();
    f.double_roundtrip(client);
    f.niri().refresh_layout();
}

fn close(f: &mut Fixture, client: ClientId, surface: &WlSurface) {
    f.client(client).window(surface).xdg_toplevel.destroy();
    f.double_roundtrip(client);
    f.niri().refresh_layout();
}

fn column_width(f: &mut Fixture, window: &Window) -> Option<(f64, bool)> {
    f.niri()
        .layout
        .workspaces()
        .find(|(_, _, ws)| ws.has_window(window))
        .unwrap()
        .2
        .normal_column_width(window)
}

fn column_proportion(f: &mut Fixture, window: &Window) -> Option<f64> {
    column_width(f, window).map(|(proportion, _)| proportion)
}

fn floating(f: &mut Fixture, key: &str) -> Option<(i32, i32)> {
    f.niri()
        .window_size_memory
        .record(key)
        .and_then(|r| r.floating)
        .map(|s| (s.width, s.height))
}

#[test]
fn window_size_memory_restores_floating_size_across_sessions() {
    let shell = xshell::Shell::new().unwrap();
    let dir = shell.create_temp_dir().unwrap();
    let path = dir.path().join("window-sizes.json");

    let mut f = setup(Some(path.clone()));
    let client = f.add_client();
    let (surface, _, _) = open(&mut f, client, "float");
    resize_floating(&mut f, client, &surface, 640, 400);
    close(&mut f, client, &surface);
    assert_eq!(floating(&mut f, "test.app"), Some((640, 400)));
    drop(f);

    let mut f = setup(Some(path));
    let client = f.add_client();
    let (_, _, size) = open(&mut f, client, "float");
    assert_eq!(size, (640, 400));
}

#[test]
fn window_size_memory_resizes_clients_that_ignore_the_initial_size() {
    let mut f = setup(None);
    let client = f.add_client();
    let (surface, _, _) = open(&mut f, client, "float");
    resize_floating(&mut f, client, &surface, 640, 400);
    close(&mut f, client, &surface);

    // Map at a self-chosen size, like Steam through Xwayland.
    let w = f.client(client).create_window();
    let surface = w.surface.clone();
    w.xdg_toplevel.set_app_id("test.app".into());
    w.set_title("float");
    w.commit();
    f.double_roundtrip(client);
    let w = f.client(client).window(&surface);
    assert_eq!(w.configures_received.last().unwrap().1.size, (640, 400));
    let _ = w.recent_configures();
    w.attach_new_buffer();
    w.set_size(1010, 600);
    w.ack_last_and_commit();
    f.double_roundtrip(client);

    let w = f.client(client).window(&surface);
    let sizes: Vec<_> = w.recent_configures().map(|c| c.size).collect();
    assert_eq!(sizes.last(), Some(&(640, 400)), "{sizes:?}");
    // The self-chosen size is not learned.
    assert_eq!(floating(&mut f, "test.app"), Some((640, 400)));
}

#[test]
fn window_size_memory_restores_tiled_column_width() {
    let shell = xshell::Shell::new().unwrap();
    let dir = shell.create_temp_dir().unwrap();
    let path = dir.path().join("window-sizes.json");

    let mut f = setup(Some(path.clone()));
    let client = f.add_client();
    let (surface, window, _) = open(&mut f, client, "main");
    f.niri().layout.set_column_width(SizeChange::SetProportion(75.));
    f.double_roundtrip(client);
    f.niri().refresh_layout();
    let proportion = column_proportion(&mut f, &window).unwrap();
    assert!((proportion - 0.75).abs() < 1e-6, "{proportion}");
    close(&mut f, client, &surface);
    drop(f);

    let mut f = setup(Some(path));
    let client = f.add_client();
    let (_, window, _) = open(&mut f, client, "main");
    let proportion = column_proportion(&mut f, &window).unwrap();
    assert!((proportion - 0.75).abs() < 1e-6, "{proportion}");
}

#[test]
fn window_size_memory_restores_full_width_column() {
    let shell = xshell::Shell::new().unwrap();
    let dir = shell.create_temp_dir().unwrap();
    let path = dir.path().join("window-sizes.json");

    let mut f = setup(Some(path.clone()));
    let client = f.add_client();
    let (surface, window, _) = open(&mut f, client, "main");
    // Title bar double-click and maximize buttons toggle full width this way.
    f.niri().layout.toggle_full_width_for_window(&window);
    f.double_roundtrip(client);
    f.niri().refresh_layout();
    assert_eq!(column_width(&mut f, &window), Some((0.5, true)));
    close(&mut f, client, &surface);
    drop(f);

    let mut f = setup(Some(path));
    let client = f.add_client();
    let (_, window, size) = open(&mut f, client, "main");
    assert_eq!(size.0, 1920 - 2 * 16, "opens at full width");
    assert_eq!(column_width(&mut f, &window), Some((0.5, true)));
    // Leaving full width returns to the remembered width.
    f.niri().layout.toggle_full_width_for_window(&window);
    assert_eq!(column_width(&mut f, &window), Some((0.5, false)));
}

#[test]
fn window_size_memory_groups_and_exclusions_are_separate() {
    let mut f = setup(None);
    let client = f.add_client();

    // A grouped auxiliary window starts from its rule default, then keeps its own record.
    let (surface, _, size) = open(&mut f, client, "dialog");
    assert_eq!(size, (900, 520));
    resize_floating(&mut f, client, &surface, 700, 450);
    close(&mut f, client, &surface);
    assert_eq!(floating(&mut f, "test.app#dialog"), Some((700, 450)));
    assert_eq!(floating(&mut f, "test.app"), None);
    let (_, _, size) = open(&mut f, client, "dialog");
    assert_eq!(size, (700, 450));

    // Excluded windows are neither restored nor learned.
    let (surface, _, _) = open(&mut f, client, "excluded");
    resize_floating(&mut f, client, &surface, 500, 300);
    close(&mut f, client, &surface);
    assert_eq!(floating(&mut f, "test.app"), None);
}
