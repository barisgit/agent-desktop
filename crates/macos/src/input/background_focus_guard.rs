use agent_desktop_core::BackgroundFocusGuard;
use std::time::Duration;

/// How long the guard keeps watching after the last event: long enough to
/// catch an app that activates itself on the next run-loop turn.
pub(crate) const GUARD_WINDOW: Duration = Duration::from_millis(400);
pub(crate) const GUARD_POLL: Duration = Duration::from_millis(25);

/// Restores are retried a few times at most so a target that keeps
/// re-activating cannot turn the guard into a focus tug-of-war.
const MAX_INTERVENTIONS: u32 = 3;

/// Side effects the guard needs, injected so the decision logic runs in
/// tests against a scripted clock and frontmost sequence.
pub(crate) trait GuardIo {
    fn now(&mut self) -> Duration;
    fn frontmost(&mut self) -> Option<i32>;
    /// Makes `pid` frontmost again; `false` when that did not succeed.
    fn restore(&mut self, pid: i32) -> bool;
    fn sleep(&mut self, duration: Duration);
}

/// Tracks whether the user's frontmost app was displaced during delivery.
///
/// Only the user's own app is ever restored; the guard never touches the
/// target, so it cannot itself cause an activation.
#[derive(Debug)]
pub(crate) struct FocusGuard {
    user_pid: i32,
    steal_started: Option<Duration>,
    max_steal: Duration,
    interventions: u32,
}

impl FocusGuard {
    pub(crate) fn new(user_pid: i32) -> Self {
        Self {
            user_pid,
            steal_started: None,
            max_steal: Duration::ZERO,
            interventions: 0,
        }
    }

    /// Takes one frontmost sample and restores the user's app if another app
    /// holds the front. An unreadable sample is ignored rather than treated
    /// as a steal.
    pub(crate) fn sample(&mut self, io: &mut impl GuardIo) {
        let now = io.now();
        match io.frontmost() {
            Some(pid) if pid != self.user_pid => {
                let started = *self.steal_started.get_or_insert(now);
                self.max_steal = self.max_steal.max(now.saturating_sub(started));
                if self.interventions < MAX_INTERVENTIONS {
                    self.interventions += 1;
                    io.restore(self.user_pid);
                }
            }
            Some(_) => {
                if let Some(started) = self.steal_started.take() {
                    self.max_steal = self.max_steal.max(now.saturating_sub(started));
                }
            }
            None => {}
        }
    }

    /// Samples every [`GUARD_POLL`] until `window` has elapsed.
    pub(crate) fn watch(&mut self, io: &mut impl GuardIo, window: Duration) {
        let until = io.now() + window;
        loop {
            self.sample(io);
            let now = io.now();
            if now >= until {
                return;
            }
            io.sleep(GUARD_POLL.min(until - now));
        }
    }

    /// `restored` is true only when the guard intervened and the user's app
    /// is frontmost in the final sample.
    pub(crate) fn finish(&self, final_frontmost: Option<i32>) -> BackgroundFocusGuard {
        BackgroundFocusGuard {
            interventions: self.interventions,
            restored: self.interventions > 0 && final_frontmost == Some(self.user_pid),
            max_steal_ms: u64::try_from(self.max_steal.as_millis()).unwrap_or(u64::MAX),
        }
    }
}

#[cfg(test)]
#[path = "background_focus_guard_tests.rs"]
mod tests;
