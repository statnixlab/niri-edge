use niri_config::Config;
use niri_ipc::{PositionChange, SizeChange};
use smithay::backend::input::{InputEvent, InputTime, KeyState};
use smithay::desktop::Window;
use smithay::reexports::wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;
use smithay::utils::{Logical, Point, Size};
use wayland_client::protocol::{wl_pointer, wl_surface::WlSurface};

use super::client::ClientId;
use super::test_input_backend::{TestInputBackend, TestKeyboardKeyEvent};
use super::Fixture;
use crate::layout::LayoutElement as _;

struct Harness {
    f: Fixture,
    id: ClientId,
    surface: WlSurface,
    pointer: ZwlrVirtualPointerV1,
    acked: u32,
}

impl Harness {
    fn new(enabled: bool) -> Self {
        let flags = if enabled {
            "floating-windows-detach-on-mod-drag; floating-windows-restore-normal-size;"
        } else {
            ""
        };
        let config = Config::parse_mem(&format!(
            r#"
            layout {{
                {flags}
                floating-windows-edge-snap
                floating-windows-match-tiling-top
                titlebar-double-click-to-column
                gaps 4
                struts {{ top 30; }}
                border {{ off; }}
            }}
            animations {{ off; }}
            binds {{
                Mod+V {{ toggle-window-floating; }}
            }}
        "#
        ))
        .unwrap();
        let mut f = Fixture::with_config(config);
        f.add_output(1, (1280, 720));
        let id = f.add_client();
        let window = f.client(id).create_window();
        let surface = window.surface.clone();
        window.commit();
        f.roundtrip(id);
        let window = f.client(id).window(&surface);
        let (acked, cfg) = window.configures_received.last().unwrap();
        let acked = *acked;
        window.attach_new_buffer();
        window.set_size(cfg.size.0.max(1) as u16, cfg.size.1.max(1) as u16);
        window.ack_last_and_commit();
        f.double_roundtrip(id);
        let pointer = f.client(id).state.virtual_pointer();
        let mut rv = Self {
            f,
            id,
            surface,
            pointer,
            acked,
        };
        rv.flush();
        rv
    }

    fn flush(&mut self) {
        // Acknowledge only new configures; a motion/frame need not produce a configure.
        for _ in 0..5 {
            self.f.double_roundtrip(self.id);
            let window = self.f.client(self.id).window(&self.surface);
            let (serial, cfg) = window.configures_received.last().unwrap();
            if *serial == self.acked {
                break;
            }
            self.acked = *serial;
            window.set_size(cfg.size.0.max(1) as u16, cfg.size.1.max(1) as u16);
            window.ack_last_and_commit();
        }
        self.f.niri_complete_animations();
    }

    fn key(&mut self, code: u32, pressed: bool) {
        self.f
            .niri_state()
            .process_input_event(InputEvent::<TestInputBackend>::Keyboard {
                event: TestKeyboardKeyEvent {
                    time: InputTime::from_micros(0),
                    code: code.into(),
                    state: if pressed {
                        KeyState::Pressed
                    } else {
                        KeyState::Released
                    },
                    count: 1,
                },
            });
        self.flush();
    }

    fn motion(&mut self, x: u32, y: u32) {
        self.pointer.motion_absolute(0, x, y, 1280, 720);
        self.pointer.frame();
        self.flush();
    }

    fn button(&mut self, pressed: bool) {
        self.pointer.button(
            0,
            0x110,
            if pressed {
                wl_pointer::ButtonState::Pressed
            } else {
                wl_pointer::ButtonState::Released
            },
        );
        self.pointer.frame();
        self.flush();
    }

    fn toggle(&mut self) {
        self.key(133, true); // Super_L
        self.key(55, true); // V
        self.key(55, false);
        self.key(133, false);
    }

    fn win(&mut self) -> Window {
        self.f
            .niri()
            .layout
            .windows()
            .next()
            .unwrap()
            .1
            .window
            .clone()
    }

    fn geometry(&mut self) -> (bool, Size<i32, Logical>, Point<f64, Logical>) {
        self.flush();
        let niri = self.f.niri();
        let win = niri.layout.windows().next().unwrap().1;
        let floating = win.is_floating();
        let size = win.size();
        let surface = win.toplevel().wl_surface().clone();
        let pos = niri.layout.window_render_location(&surface).unwrap();
        (floating, size, pos)
    }

    fn ordinary_size(&mut self, size: (u16, u16)) {
        let window = self.f.client(self.id).window(&self.surface);
        window.set_size(size.0, size.1);
        window.commit();
        self.flush();
    }

    fn start_drag(&mut self, shift: bool) {
        self.motion(200, 200);
        self.key(133, true);
        if shift {
            self.key(50, true);
        }
        self.button(true);
    }
}

#[test]
fn floating_detach_threshold_and_modifier_release_keep_grab_anchor() {
    let mut h = Harness::new(true);
    let original = h.geometry();
    assert!(!original.0);
    h.start_drag(false);
    h.motion(204, 200);
    assert_eq!(h.geometry(), original);
    h.motion(212, 200);
    let detached = h.geometry();
    assert!(detached.0);
    assert_eq!(detached.1, Size::from((634, 477)));
    let ratio_x = (200. - original.2.x) / f64::from(original.1.w);
    let ratio_y = (200. - original.2.y) / f64::from(original.1.h);
    assert!((212. - detached.2.x - f64::from(detached.1.w) * ratio_x).abs() < 1.);
    assert!((200. - detached.2.y - f64::from(detached.1.h) * ratio_y).abs() < 1.);
    h.key(133, false);
    h.motion(312, 250);
    assert!(h.geometry().0);
    h.button(false);
    assert!(h.geometry().0);
}

#[test]
fn floating_detach_click_without_drag_keeps_tiling() {
    let mut h = Harness::new(true);
    let before = h.geometry();
    h.start_drag(false);
    h.button(false);
    h.key(133, false);
    assert_eq!(h.geometry(), before);
}

#[test]
fn floating_detach_shift_keeps_reordering_after_modifiers_are_released() {
    let mut h = Harness::new(true);
    h.start_drag(true);
    h.key(50, false);
    h.key(133, false);
    h.motion(550, 220);
    assert!(!h.geometry().0);
    h.button(false);
    assert!(!h.geometry().0);
    assert_eq!(h.geometry().1.h, 682);
}

#[test]
fn floating_detach_edge_snap_and_win_v_restore_compact_size() {
    for x in [0, 1279] {
        let mut h = Harness::new(true);
        h.start_drag(false);
        h.motion(212, 200);
        h.motion(x, 200);
        h.key(133, false);
        h.button(false);
        assert!(!h.geometry().0);
        assert_eq!(h.geometry().1, Size::from((634, 682)));
        h.toggle();
        assert!(h.geometry().0);
        assert_eq!(h.geometry().1, Size::from((634, 477)));
    }
}

#[test]
fn floating_detach_win_v_centers_first_float_and_remembers_manual_geometry() {
    let mut h = Harness::new(true);
    let before = h.geometry();
    h.toggle();
    let first = h.geometry();
    assert!(first.0);
    assert_eq!(first.1, Size::from((634, 477)));
    assert!(
        (before.2.y + f64::from(before.1.h) / 2. - first.2.y - f64::from(first.1.h) / 2.).abs()
            < 1.
    );
    h.ordinary_size((1000, 550));
    let win = h.win();
    h.f.niri().layout.move_floating_window(
        Some(&win),
        PositionChange::SetFixed(200.),
        PositionChange::SetFixed(150.),
        false,
    );
    let resized = h.geometry();
    h.toggle();
    h.toggle();
    assert_eq!(h.geometry(), resized);
}

#[test]
fn floating_detach_expansion_and_native_maximize_do_not_replace_normal_memory() {
    let mut h = Harness::new(true);
    h.toggle();
    h.ordinary_size((500, 320));
    let ordinary = h.geometry();
    let win = h.win();
    h.f.niri().layout.toggle_floating_expanded(&win);
    assert_eq!(h.geometry().1, Size::from((1272, 682)));
    h.toggle();
    h.toggle();
    assert_eq!(h.geometry(), ordinary);
    h.f.client(h.id).window(&h.surface).set_maximized();
    h.flush();
    h.f.client(h.id).window(&h.surface).unset_maximized();
    h.flush();
    assert_eq!(h.geometry(), ordinary);
}

#[test]
fn floating_detach_preserves_small_windows_and_application_minimum() {
    let mut h = Harness::new(true);
    let win = h.win();
    h.f.niri()
        .layout
        .set_window_width(Some(&win), SizeChange::SetFixed(300));
    h.f.niri()
        .layout
        .set_window_height(Some(&win), SizeChange::SetFixed(240));
    h.flush();
    h.toggle();
    assert_eq!(h.geometry().1, Size::from((300, 240)));
    let mut h = Harness::new(true);
    let window = h.f.client(h.id).window(&h.surface);
    window.xdg_toplevel.set_min_size(500, 600);
    window.commit();
    h.flush();
    h.toggle();
    assert_eq!(h.geometry().1.h, 600);
}

#[test]
fn floating_detach_disabled_retains_existing_behavior() {
    let mut h = Harness::new(false);
    h.start_drag(false);
    h.motion(212, 200);
    assert!(!h.geometry().0);
    h.button(false);
    h.key(133, false);
    h.toggle();
    assert_eq!(h.geometry().1.h, 682);
}

#[test]
fn floating_detach_first_full_width_window_is_capped_in_both_dimensions() {
    let mut h = Harness::new(true);
    h.f.niri().layout.toggle_full_width();
    h.flush();
    assert_eq!(h.geometry().1, Size::from((1272, 682)));
    h.toggle();
    assert_eq!(h.geometry().1, Size::from((890, 477)));
}
