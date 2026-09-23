use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::{asset::character_pack::CharacterPack, config::character::Facing};

// ponytail: fixed timing keeps this phase small; Phase 4 adds weighted transitions.
const IDLE_DURATION: Duration = Duration::from_secs(3);
const WALK_DURATION: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Idle,
    Walk,
}

struct Motion {
    frames: Vec<PathBuf>,
    fps: u16,
    looped: bool,
}

pub struct Animation {
    idle: Motion,
    walk: Motion,
    walk_speed: f32,
    source_facing: Facing,
    facing: Facing,
    mode: Mode,
    mode_started: Instant,
    last_tick: Instant,
    frame_index: usize,
    subpixel: f32,
}

pub struct Tick {
    pub dx: i32,
    pub redraw: bool,
    pub mode_changed: bool,
}

impl Animation {
    pub fn new(pack: &CharacterPack, now: Instant) -> Result<Self, String> {
        let idle = pack.resolve_motion("idle")?;
        let walk = pack.resolve_motion("walk")?;
        Ok(Self {
            idle: Motion {
                frames: idle.frames.clone(),
                fps: idle.config.fps,
                looped: idle.config.looped,
            },
            walk: Motion {
                frames: walk.frames.clone(),
                fps: walk.config.fps,
                looped: walk.config.looped,
            },
            walk_speed: pack.config.motion_or_default("walk").speed.unwrap_or(90.0),
            source_facing: pack.config.default_facing,
            facing: pack.config.default_facing,
            mode: Mode::Idle,
            mode_started: now,
            last_tick: now,
            frame_index: 0,
            subpixel: 0.0,
        })
    }

    fn motion(&self) -> &Motion {
        match self.mode {
            Mode::Idle => &self.idle,
            Mode::Walk => &self.walk,
        }
    }

    pub fn frame(&self) -> &Path {
        &self.motion().frames[self.frame_index]
    }

    pub fn should_flip(&self) -> bool {
        self.facing != self.source_facing
    }

    pub fn reverse(&mut self) {
        self.facing = match self.facing {
            Facing::Left => Facing::Right,
            Facing::Right => Facing::Left,
        };
    }

    pub fn reset_idle(&mut self, now: Instant) -> bool {
        let redraw = self.frame() != self.idle.frames[0];
        self.mode = Mode::Idle;
        self.mode_started = now;
        self.last_tick = now;
        self.frame_index = 0;
        self.subpixel = 0.0;
        redraw
    }

    pub fn interval_ms(&self) -> u32 {
        if self.mode == Mode::Idle && self.idle.frames.len() == 1 {
            return IDLE_DURATION.as_millis() as u32;
        }
        let frame_ms = (1000 / u32::from(self.motion().fps)).max(1);
        if self.mode == Mode::Walk {
            frame_ms.min(33)
        } else {
            frame_ms
        }
    }

    pub fn tick(&mut self, now: Instant) -> Tick {
        let previous_mode = self.mode;
        let duration = if self.mode == Mode::Idle {
            IDLE_DURATION
        } else {
            WALK_DURATION
        };
        if now.duration_since(self.mode_started) >= duration {
            self.mode = if self.mode == Mode::Idle {
                Mode::Walk
            } else {
                Mode::Idle
            };
            self.mode_started = now;
            self.frame_index = 0;
            self.subpixel = 0.0;
        }

        let mode_changed = self.mode != previous_mode;
        let elapsed = now.duration_since(self.mode_started);
        let next_frame = frame_index(
            elapsed,
            self.motion().fps,
            self.motion().frames.len(),
            self.motion().looped,
        );
        let redraw = mode_changed || next_frame != self.frame_index;
        self.frame_index = next_frame;

        let mut dx = 0;
        if self.mode == Mode::Walk && !mode_changed {
            let distance =
                self.walk_speed * now.duration_since(self.last_tick).as_secs_f32() + self.subpixel;
            let pixels = distance.floor() as i32;
            self.subpixel = distance - pixels as f32;
            dx = if self.facing == Facing::Right {
                pixels
            } else {
                -pixels
            };
        }
        self.last_tick = now;
        Tick {
            dx,
            redraw,
            mode_changed,
        }
    }
}

fn frame_index(elapsed: Duration, fps: u16, count: usize, looped: bool) -> usize {
    let index = elapsed.as_millis() * u128::from(fps) / 1000;
    if looped {
        (index % count as u128) as usize
    } else {
        index.min((count - 1) as u128) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_advance_and_loop_at_configured_fps() {
        assert_eq!(frame_index(Duration::from_millis(249), 4, 3, true), 0);
        assert_eq!(frame_index(Duration::from_millis(250), 4, 3, true), 1);
        assert_eq!(frame_index(Duration::from_millis(750), 4, 3, true), 0);
        assert_eq!(frame_index(Duration::from_millis(750), 4, 3, false), 2);
    }

    #[test]
    fn idle_and_walk_transition_without_an_idle_frame_loop() {
        let now = Instant::now();
        let mut animation = Animation {
            idle: Motion {
                frames: vec!["idle".into()],
                fps: 4,
                looped: true,
            },
            walk: Motion {
                frames: vec!["walk".into()],
                fps: 8,
                looped: true,
            },
            walk_speed: 90.0,
            source_facing: Facing::Right,
            facing: Facing::Right,
            mode: Mode::Idle,
            mode_started: now,
            last_tick: now,
            frame_index: 0,
            subpixel: 0.0,
        };
        assert_eq!(animation.interval_ms(), 3000);
        assert!(animation.tick(now + IDLE_DURATION).mode_changed);
        assert_eq!(animation.frame(), Path::new("walk"));
        assert!(
            animation
                .tick(now + IDLE_DURATION + Duration::from_millis(100))
                .dx
                > 0
        );
        assert!(
            animation
                .tick(now + IDLE_DURATION + WALK_DURATION)
                .mode_changed
        );
        assert_eq!(animation.frame(), Path::new("idle"));
    }
}
