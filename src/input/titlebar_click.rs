use std::time::Duration;

use smithay::utils::{Logical, Point};

use super::DOUBLE_CLICK_TIME;
use crate::window::mapped::MappedId;

const CLICK_DISTANCE: f64 = 6.;

/// Correlates asynchronous client requests with a completed pointer double click.
///
/// A completed pair survives pointer movement while the client is processing it.
/// New physical input still clears it, and requests from single button clicks
/// consume their candidate immediately so rapid button clicks stay native.
#[derive(Debug, Default)]
pub struct TitlebarClicks {
    last: Option<Click>,
    completed: Option<CompletedClick>,
    handled: Option<HandledRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaximizeRequest {
    Native,
    DoubleClick,
    RepeatedDoubleClick,
}

#[derive(Debug)]
struct Click {
    window: MappedId,
    position: Point<f64, Logical>,
    time: Duration,
    released: bool,
    double: bool,
}

#[derive(Debug)]
struct CompletedClick {
    window: MappedId,
    time: Duration,
}

#[derive(Debug)]
struct HandledRequest {
    window: MappedId,
    maximize: bool,
    time: Duration,
}

impl TitlebarClicks {
    pub fn clear(&mut self) {
        self.last = None;
        self.completed = None;
        self.handled = None;
    }

    pub fn motion(&mut self, position: Point<f64, Logical>) {
        if self.last.as_ref().is_some_and(|last| {
            let delta = position - last.position;
            delta.x * delta.x + delta.y * delta.y > CLICK_DISTANCE * CLICK_DISTANCE
        }) {
            // Motion can invalidate a first-click candidate, but cannot undo an
            // already completed pair or the client request it already produced.
            self.last = None;
        }
    }

    pub fn press(&mut self, window: MappedId, position: Point<f64, Logical>, time: Duration) {
        self.completed = None;
        self.handled = None;
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
        if double {
            self.completed = Some(CompletedClick { window, time });
        }
    }

    pub fn release(&mut self, position: Point<f64, Logical>) {
        self.motion(position);
        if let Some(last) = &mut self.last {
            last.released = true;
        }
    }

    pub fn classify_request(
        &mut self,
        window: MappedId,
        time: Duration,
        maximize: bool,
    ) -> MaximizeRequest {
        if self
            .completed
            .as_ref()
            .is_some_and(|click| time.saturating_sub(click.time) > DOUBLE_CLICK_TIME)
        {
            self.completed = None;
        }
        if self
            .handled
            .as_ref()
            .is_some_and(|request| time.saturating_sub(request.time) > DOUBLE_CLICK_TIME)
        {
            self.handled = None;
        }
        if self
            .handled
            .as_ref()
            .is_some_and(|request| request.window == window && request.maximize == maximize)
        {
            // Repeated requests for the same gesture must neither toggle again
            // nor fall through to native maximization. Do not extend the timeout.
            return MaximizeRequest::RepeatedDoubleClick;
        }
        if self
            .last
            .as_ref()
            .is_some_and(|click| click.window == window)
        {
            self.last = None;
        }
        if self
            .completed
            .as_ref()
            .is_some_and(|click| click.window == window)
        {
            self.completed = None;
            self.handled = Some(HandledRequest {
                window,
                maximize,
                time,
            });
            return MaximizeRequest::DoubleClick;
        }
        if self
            .handled
            .as_ref()
            .is_some_and(|request| request.window == window)
        {
            self.handled = None;
        }
        // An unrelated client's request must not consume this window's pair.
        MaximizeRequest::Native
    }

    #[cfg(test)]
    fn take_double_click(&mut self, window: MappedId, time: Duration) -> bool {
        self.classify_request(window, time, true) == MaximizeRequest::DoubleClick
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

    #[test]
    fn completed_pair_survives_motion_and_release_before_request() {
        for released in [false, true] {
            let window = MappedId::next();
            let mut clicks = TitlebarClicks::default();
            clicks.press(window, (20., 20.).into(), ms(0));
            clicks.release((20., 20.).into());
            clicks.press(window, (21., 20.).into(), ms(100));
            if released {
                clicks.release((21., 20.).into());
            }
            clicks.motion((100., 100.).into());
            assert_eq!(
                clicks.classify_request(window, ms(150), true),
                MaximizeRequest::DoubleClick
            );
        }
    }

    #[test]
    fn duplicate_request_is_idempotent_but_new_button_click_is_native() {
        let window = MappedId::next();
        let mut clicks = TitlebarClicks::default();
        clicks.press(window, (20., 20.).into(), ms(0));
        clicks.release((20., 20.).into());
        clicks.press(window, (20., 20.).into(), ms(100));
        assert_eq!(
            clicks.classify_request(window, ms(110), true),
            MaximizeRequest::DoubleClick
        );
        clicks.release((20., 20.).into());
        clicks.motion((100., 20.).into());
        assert_eq!(
            clicks.classify_request(window, ms(120), true),
            MaximizeRequest::RepeatedDoubleClick
        );
        clicks.press(window, (100., 20.).into(), ms(130));
        assert_eq!(
            clicks.classify_request(window, ms(140), true),
            MaximizeRequest::Native
        );
    }

    #[test]
    fn unrelated_client_does_not_steal_a_completed_pair() {
        let window = MappedId::next();
        let other = MappedId::next();
        let mut clicks = TitlebarClicks::default();
        clicks.press(window, (20., 20.).into(), ms(0));
        clicks.release((20., 20.).into());
        clicks.press(window, (20., 20.).into(), ms(100));
        assert_eq!(
            clicks.classify_request(other, ms(110), true),
            MaximizeRequest::Native
        );
        assert_eq!(
            clicks.classify_request(window, ms(120), true),
            MaximizeRequest::DoubleClick
        );
        assert_eq!(
            clicks.classify_request(other, ms(130), true),
            MaximizeRequest::Native
        );
        assert_eq!(
            clicks.classify_request(window, ms(140), true),
            MaximizeRequest::RepeatedDoubleClick
        );
    }

    #[test]
    fn duplicate_cache_expires_without_extending_on_repeated_requests() {
        let window = MappedId::next();
        let mut clicks = TitlebarClicks::default();
        clicks.press(window, (20., 20.).into(), ms(0));
        clicks.release((20., 20.).into());
        clicks.press(window, (20., 20.).into(), ms(100));
        assert_eq!(
            clicks.classify_request(window, ms(110), false),
            MaximizeRequest::DoubleClick
        );
        assert_eq!(
            clicks.classify_request(window, ms(400), false),
            MaximizeRequest::RepeatedDoubleClick
        );
        assert_eq!(
            clicks.classify_request(window, ms(511), false),
            MaximizeRequest::Native
        );
    }
}
