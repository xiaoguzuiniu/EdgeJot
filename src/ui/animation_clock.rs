//! Compositor-paced wakeups; animation timestamps and rendering stay on the UI thread.

use std::{
    io,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    Graphics::Dwm::DwmFlush,
    UI::WindowsAndMessaging::PostMessageW,
};

const MIN_FRAME_INTERVAL: Duration = Duration::from_millis(2);
const FALLBACK_FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Posts at most one unacknowledged frame message, without accessing UI state.
///
/// The owner must drop this clock before destroying its window. Dropping joins
/// the worker, but does not remove messages already in the window's queue.
pub(super) struct AnimationClock {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct State {
    active: bool,
    pending: bool,
    shutdown: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> Option<MutexGuard<'_, State>> {
        self.check_lock(self.state.lock())
    }

    fn check_lock<'a>(
        &self,
        result: std::sync::LockResult<MutexGuard<'a, State>>,
    ) -> Option<MutexGuard<'a, State>> {
        match result {
            Ok(state) => Some(state),
            Err(error) => {
                eprintln!("Animation clock mutex poisoned; stopping frame scheduler");
                let mut state = error.into_inner();
                state.shutdown = true;
                state.active = false;
                self.wake.notify_all();
                None
            }
        }
    }
}

impl AnimationClock {
    /// Creates an initially inactive clock. `message` should be a dedicated
    /// application message handled only by the owning window's UI thread.
    pub(super) fn new(window: HWND, message: u32) -> io::Result<Self> {
        Self::with_backend(NativeBackend {
            window: window.0 as usize,
            message,
            fallback: false,
        })
    }

    fn with_backend(backend: impl FrameBackend) -> io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("animation-clock".into())
            .spawn(move || run_worker(worker_shared, backend))?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    /// Enables wakeups while any animation is running. Disabling does not
    /// discard an already queued message: it must still be acknowledged.
    pub(super) fn set_active(&self, active: bool) {
        if let Some(mut state) = self.shared.lock() {
            if state.shutdown {
                return;
            }
            state.active = active;
            self.shared.wake.notify_all();
        }
    }

    /// Acknowledge each received frame message, including after deactivation.
    /// Call after processing that frame and updating the active animation state.
    pub(super) fn frame_handled(&self) {
        if let Some(mut state) = self.shared.lock() {
            state.pending = false;
            self.shared.wake.notify_all();
        }
    }
}

impl Drop for AnimationClock {
    fn drop(&mut self) {
        if let Some(mut state) = self.shared.lock() {
            state.shutdown = true;
            self.shared.wake.notify_all();
        }
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                eprintln!("Animation clock worker panicked");
            }
        }
    }
}

trait FrameBackend: Send + 'static {
    /// Wait for the compositor and return any additional interruptible delay.
    fn pace(&mut self) -> Duration;
    /// Report/log native errors and return false to stop the scheduler.
    fn post_frame(&mut self) -> bool;

    #[cfg(test)]
    fn parked(&mut self, _active: bool, _pending: bool) {}
}

struct NativeBackend {
    // Capture an integer, not a borrowed UI object or a Send wrapper around HWND.
    window: usize,
    message: u32,
    fallback: bool,
}

impl FrameBackend for NativeBackend {
    fn pace(&mut self) -> Duration {
        if self.fallback {
            return FALLBACK_FRAME_INTERVAL;
        }
        let started = Instant::now();
        // SAFETY: DwmFlush takes no pointers and only waits for compositor work.
        if let Err(error) = unsafe { DwmFlush() } {
            eprintln!("Animation clock DwmFlush failed: {error}; using 16 ms fallback pacing");
            self.fallback = true;
            return FALLBACK_FRAME_INTERVAL;
        }
        // Guard against a compositor returning immediately without restricting
        // normal 120/144/240 Hz intervals. This delay is interruptible on stop.
        MIN_FRAME_INTERVAL.saturating_sub(started.elapsed())
    }

    fn post_frame(&mut self) -> bool {
        let window = HWND(self.window as *mut core::ffi::c_void);
        // SAFETY: The owner keeps this HWND alive until the worker is joined.
        // The message contains no pointers and never performs UI work here.
        match unsafe { PostMessageW(Some(window), self.message, WPARAM(0), LPARAM(0)) } {
            Ok(()) => true,
            Err(error) => {
                eprintln!("Animation clock PostMessageW failed: {error}; stopping frame scheduler");
                false
            }
        }
    }
}

fn run_worker(shared: Arc<Shared>, mut backend: impl FrameBackend) {
    loop {
        let Some(mut state) = shared.lock() else {
            return;
        };
        while !state.shutdown && (!state.active || state.pending) {
            #[cfg(test)]
            backend.parked(state.active, state.pending);
            let Some(next) = shared.check_lock(shared.wake.wait(state)) else {
                return;
            };
            state = next;
        }
        if state.shutdown {
            return;
        }
        drop(state);

        let delay = backend.pace();
        let deadline = Instant::now() + delay;
        let Some(mut state) = shared.lock() else {
            return;
        };
        while !state.shutdown && state.active && !state.pending {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let result = shared
                .wake
                .wait_timeout(state, remaining)
                .map(|(state, _)| state)
                .map_err(|error| std::sync::PoisonError::new(error.into_inner().0));
            let Some(next) = shared.check_lock(result) else {
                return;
            };
            state = next;
        }
        if state.shutdown {
            return;
        }
        if !state.active || state.pending {
            continue;
        }
        // Serialize posting with deactivation/acknowledgement so a fast UI
        // cannot acknowledge a message before its pending flag is recorded.
        state.pending = true;
        if !backend.post_frame() {
            state.pending = false;
            state.active = false;
            state.shutdown = true;
            shared.wake.notify_all();
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{self, Receiver, Sender};

    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Parked { active: bool, pending: bool },
        Pacing,
        Posted,
    }

    struct MockBackend {
        events: Sender<Event>,
        frames: Receiver<()>,
        post_succeeds: bool,
    }

    impl FrameBackend for MockBackend {
        fn pace(&mut self) -> Duration {
            self.events.send(Event::Pacing).unwrap();
            // A bounded wait also keeps teardown finite if an assertion fails.
            self.frames.recv_timeout(TEST_TIMEOUT).unwrap();
            Duration::ZERO
        }

        fn post_frame(&mut self) -> bool {
            self.events.send(Event::Posted).unwrap();
            self.post_succeeds
        }

        fn parked(&mut self, active: bool, pending: bool) {
            self.events.send(Event::Parked { active, pending }).unwrap();
        }
    }

    fn clock(post_succeeds: bool) -> (AnimationClock, Receiver<Event>, Sender<()>) {
        let (events_tx, events_rx) = mpsc::channel();
        let (frames_tx, frames_rx) = mpsc::channel();
        let clock = AnimationClock::with_backend(MockBackend {
            events: events_tx,
            frames: frames_rx,
            post_succeeds,
        })
        .unwrap();
        (clock, events_rx, frames_tx)
    }

    fn expect(events: &Receiver<Event>, expected: Event) {
        loop {
            let actual = events.recv_timeout(TEST_TIMEOUT).unwrap();
            if actual == expected {
                return;
            }
            // Condvars may wake spuriously; another park is not another frame.
            if !matches!(actual, Event::Parked { .. }) {
                panic!("expected {expected:?}, received {actual:?}");
            }
        }
    }

    fn expect_parked(events: &Receiver<Event>, active: bool, pending: bool) {
        expect(events, Event::Parked { active, pending });
    }

    #[test]
    fn initially_inactive_and_drop_wakes_sleeping_worker() {
        let (clock, events, _frames) = clock(true);
        expect_parked(&events, false, false);
        drop(clock);
        assert!(
            events
                .iter()
                .all(|event| matches!(event, Event::Parked { .. }))
        );
    }

    #[test]
    fn stop_during_pacing_suppresses_frame_and_restart_resumes() {
        let (clock, events, frames) = clock(true);
        expect_parked(&events, false, false);
        clock.set_active(true);
        expect(&events, Event::Pacing);
        clock.set_active(false);
        frames.send(()).unwrap();
        expect_parked(&events, false, false);

        clock.set_active(true);
        expect(&events, Event::Pacing);
        frames.send(()).unwrap();
        expect(&events, Event::Posted);
        expect_parked(&events, true, true);
        drop(clock);
    }

    #[test]
    fn only_one_notification_is_pending_until_acknowledged() {
        let (clock, events, frames) = clock(true);
        clock.set_active(true);
        expect(&events, Event::Pacing);
        frames.send(()).unwrap();
        expect(&events, Event::Posted);
        expect_parked(&events, true, true);

        clock.set_active(true);
        expect_parked(&events, true, true);
        clock.frame_handled();
        expect(&events, Event::Pacing);
        frames.send(()).unwrap();
        expect(&events, Event::Posted);
        expect_parked(&events, true, true);
        drop(clock);
    }

    #[test]
    fn stop_start_preserves_pending_and_inactive_ack_does_not_start_pacing() {
        let (clock, events, frames) = clock(true);
        clock.set_active(true);
        expect(&events, Event::Pacing);
        frames.send(()).unwrap();
        expect(&events, Event::Posted);
        expect_parked(&events, true, true);

        clock.set_active(false);
        expect_parked(&events, false, true);
        clock.set_active(true);
        expect_parked(&events, true, true);
        clock.set_active(false);
        expect_parked(&events, false, true);
        clock.frame_handled();
        expect_parked(&events, false, false);
        drop(clock);
    }

    #[test]
    fn failed_post_stops_worker_without_retrying() {
        let (clock, events, frames) = clock(false);
        clock.set_active(true);
        expect(&events, Event::Pacing);
        frames.send(()).unwrap();
        expect(&events, Event::Posted);
        // Taking this lock synchronizes with completion of the failed post.
        assert!(clock.shared.lock().unwrap().shutdown);
        clock.set_active(true);
        clock.frame_handled();
        drop(clock);
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn poisoned_mutex_stops_and_wakes_worker() {
        let (clock, events, _frames) = clock(true);
        expect_parked(&events, false, false);
        let shared = Arc::clone(&clock.shared);
        let poisoned = std::panic::catch_unwind(move || {
            let _state = shared.state.lock().unwrap();
            panic!("deliberately poison scheduler mutex");
        });
        assert!(poisoned.is_err());
        clock.set_active(true);
        drop(clock);
    }
}
