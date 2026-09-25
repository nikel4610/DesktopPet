use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const IDLE_DECISION_INTERVAL: Duration = Duration::from_secs(3);
const WALK_DECISION_INTERVAL: Duration = Duration::from_secs(2);
const MIN_IDLE_PAUSE_MS: u64 = 1800;
const IDLE_PAUSE_STEP_MS: u64 = 26;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Idle,
    Walk,
}

pub struct Behavior {
    mode: Mode,
    decision_started: Instant,
    last_interaction: Instant,
    random_state: u64,
    walk_right: bool,
    walk_duration_ms: u16,
    idle_decision_interval: Duration,
}

impl Behavior {
    pub fn new(now: Instant) -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(1);
        Self::with_seed(now, seed)
    }

    pub(crate) fn with_seed(now: Instant, seed: u64) -> Self {
        Self {
            mode: Mode::Idle,
            decision_started: now,
            last_interaction: now,
            random_state: seed | 1,
            walk_right: true,
            walk_duration_ms: 0,
            idle_decision_interval: IDLE_DECISION_INTERVAL,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn walk_right(&self) -> bool {
        self.walk_right
    }

    pub fn walk_duration(&self) -> Duration {
        Duration::from_millis(u64::from(self.walk_duration_ms))
    }

    pub fn decision_interval(&self) -> Duration {
        match self.mode {
            Mode::Idle => self.idle_decision_interval,
            Mode::Walk => WALK_DECISION_INTERVAL,
        }
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        if now.duration_since(self.decision_started) < self.decision_interval() {
            return false;
        }

        self.decision_started = now;
        let inactivity = now.duration_since(self.last_interaction);
        let next = choose_next(self.mode, inactivity, self.next_percent());
        let changed = next != self.mode;
        self.mode = next;
        if changed && next == Mode::Walk {
            self.walk_right = self.next_percent() < 50;
            // Keep each excursion shorter than the two-second walk decision window.
            self.walk_duration_ms = 700 + (self.next_percent() * 10) as u16;
        } else if next == Mode::Idle {
            self.choose_idle_pause();
        }
        changed
    }

    pub fn finish_walk(&mut self, now: Instant) {
        self.mode = Mode::Idle;
        self.decision_started = now;
        self.choose_idle_pause();
    }

    pub fn reset_after_interaction(&mut self, now: Instant) {
        self.mode = Mode::Idle;
        self.decision_started = now;
        self.last_interaction = now;
        self.choose_idle_pause();
    }

    fn choose_idle_pause(&mut self) {
        let milliseconds = MIN_IDLE_PAUSE_MS + u64::from(self.next_percent()) * IDLE_PAUSE_STEP_MS;
        self.idle_decision_interval = Duration::from_millis(milliseconds);
    }

    fn next_percent(&mut self) -> u32 {
        let mut state = self.random_state;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.random_state = state;
        (state % 100) as u32
    }
}

fn choose_next(mode: Mode, inactivity: Duration, roll: u32) -> Mode {
    debug_assert!(roll < 100);
    // Longer periods without interaction favor resting. Walk remains possible.
    let rest_bonus = (inactivity.as_secs() / 30).min(4) as u32 * 5;
    let idle_weight = match mode {
        Mode::Idle => 60 + rest_bonus,
        Mode::Walk => 70 + rest_bonus,
    };
    if roll < idle_weight {
        Mode::Idle
    } else {
        Mode::Walk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_transitions_follow_state_weights() {
        assert_eq!(choose_next(Mode::Idle, Duration::ZERO, 59), Mode::Idle);
        assert_eq!(choose_next(Mode::Idle, Duration::ZERO, 60), Mode::Walk);
        assert_eq!(choose_next(Mode::Walk, Duration::ZERO, 69), Mode::Idle);
        assert_eq!(choose_next(Mode::Walk, Duration::ZERO, 70), Mode::Walk);
    }

    #[test]
    fn inactivity_increases_rest_weight_without_removing_walk() {
        let inactive = Duration::from_secs(120);
        assert_eq!(choose_next(Mode::Idle, inactive, 79), Mode::Idle);
        assert_eq!(choose_next(Mode::Idle, inactive, 80), Mode::Walk);
        assert_eq!(choose_next(Mode::Walk, inactive, 89), Mode::Idle);
        assert_eq!(choose_next(Mode::Walk, inactive, 90), Mode::Walk);
    }

    #[test]
    fn decision_waits_for_interval_and_drag_release_resets_idle() {
        let now = Instant::now();
        let seed = (1..1000)
            .find(|seed| Behavior::with_seed(now, *seed).next_percent() >= 60)
            .unwrap();
        let mut behavior = Behavior::with_seed(now, seed);
        assert!(!behavior.tick(now + IDLE_DECISION_INTERVAL - Duration::from_millis(1)));
        assert_eq!(behavior.mode(), Mode::Idle);
        assert!(behavior.tick(now + IDLE_DECISION_INTERVAL));
        assert_eq!(behavior.mode(), Mode::Walk);

        let released = now + IDLE_DECISION_INTERVAL + Duration::from_secs(1);
        behavior.reset_after_interaction(released);
        assert_eq!(behavior.mode(), Mode::Idle);
        let pause = behavior.decision_interval();
        assert!((1800..=4374).contains(&pause.as_millis()));
        assert!(!behavior.tick(released + pause - Duration::from_millis(1)));
    }

    #[test]
    fn idle_pause_varies_between_decisions() {
        let now = Instant::now();
        let mut pauses = std::collections::HashSet::new();
        for seed in 1..20 {
            let mut behavior = Behavior::with_seed(now, seed);
            behavior.finish_walk(now);
            pauses.insert(behavior.decision_interval());
        }
        assert!(pauses.len() > 1);
        assert!(
            pauses
                .iter()
                .all(|pause| (1800..=4374).contains(&pause.as_millis()))
        );
    }
}
