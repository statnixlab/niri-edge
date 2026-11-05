use std::time::Duration;

use smithay::utils::{Logical, Point};

use super::DOUBLE_CLICK_TIME;
use crate::window::mapped::MappedId;

const CLICK_DISTANCE: f64 = 6.;

/// Correlates client maximize requests with a recent pair of pointer clicks.
///
/// Wayland does not identify the widget that sent a maximize request. Consume the
/// click sequence on *every* maximize/unmaximize request: a button acts on its
/// first click, whereas a titlebar acts on the second. This also prevents two
/// quick clicks on the maximize button from being treated as a titlebar gesture.
#[derive(Debug, Default)]
pub struct TitlebarClicks {
    last: Option<Click>,
}

#[derive(Debug)]
struct Click {
    window: MappedId,
    position: Point<f64, Logical>,
    time: Duration,
    released: bool,
    double: bool,
}

impl TitlebarClicks {
    pub fn clear(&mut self) {
        self.last = None;
    }

    pub fn motion(&mut self, position: Point<f64, Logical>) {
        if self.last.as_ref().is_some_and(|last| {
            let delta = position - last.position;
            delta.x * delta.x + delta.y * delta.y > CLICK_DISTANCE * CLICK_DISTANCE
        }) {
            self.clear();
        }
    }

    pub fn press(&mut self, window: MappedId, position: Point<f64, Logical>, time: Duration) {
        self.motion(position);
        let double = self.last.as_ref().is_some_and(|last| {
            last.window == window
                && last.released
                && !last.double
                && time.saturating_sub(last.time) <= DOUBLE_CLICK_TIME
        });
        self.last = Some(Click {
            window,
            position,
            time,
            released: false,
            double,
        });
    }

    pub fn release(&mut self, position: Point<f64, Logical>) {
        self.motion(position);
        if let Some(last) = &mut self.last {
            last.released = true;
        }
    }

    pub fn take_double_click(&mut self, window: MappedId, time: Duration) -> bool {
        self.last.take().is_some_and(|last| {
            last.window == window
                && last.double
                && time.saturating_sub(last.time) <= DOUBLE_CLICK_TIME
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn titlebar_pair_is_consumed_once() {
        let window = MappedId::next();
        let mut clicks = TitlebarClicks::default();
        clicks.press(window, (20., 20.).into(), ms(0));
        clicks.release((20., 20.).into());
        clicks.press(window, (22., 21.).into(), ms(200));
        assert!(clicks.take_double_click(window, ms(210)));
        assert!(!clicks.take_double_click(window, ms(211)));
    }

    #[test]
    fn repeated_button_requests_stay_native() {
        let window = MappedId::next();
        let mut clicks = TitlebarClicks::default();
        for time in [0, 100, 200] {
            clicks.press(window, (20., 20.).into(), ms(time));
            clicks.release((20., 20.).into());
            assert!(!clicks.take_double_click(window, ms(time + 10)));
        }
    }

    #[test]
    fn rejects_drags_distant_clicks_and_other_windows() {
        for case in 0..3 {
            let window = MappedId::next();
            let mut clicks = TitlebarClicks::default();
            clicks.press(window, (20., 20.).into(), ms(0));
            if case == 0 {
                clicks.motion((80., 20.).into());
            }
            clicks.release((20., 20.).into());
            let second = if case == 2 { MappedId::next() } else { window };
            let position = if case == 1 { (80., 20.) } else { (20., 20.) };
            clicks.press(second, position.into(), ms(100));
            assert!(!clicks.take_double_click(second, ms(110)));
        }
    }

    #[test]
    fn rejects_slow_clicks_stale_requests_and_missing_release() {
        for case in 0..3 {
            let window = MappedId::next();
            let mut clicks = TitlebarClicks::default();
            clicks.press(window, (20., 20.).into(), ms(0));
            if case != 2 {
                clicks.release((20., 20.).into());
            }
            clicks.press(
                window,
                (20., 20.).into(),
                ms(if case == 0 { 500 } else { 100 }),
            );
            assert!(!clicks.take_double_click(window, ms(if case == 2 { 110 } else { 600 })));
        }
    }
}
