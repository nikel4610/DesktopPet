use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::{
    asset::character_pack::CharacterPack,
    behavior::{Behavior, Mode},
    config::character::Facing,
};

struct Motion {
    frames: Vec<PathBuf>,
    fps: u16,
    looped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    Happy,
    Special,
    Dragged,
    Fall,
    Land,
}

pub struct Animation {
    idle: Motion,
    walk: Motion,
    happy: Motion,
    special: Motion,
    dragged: Motion,
    fall: Motion,
    land: Motion,
    reaction: Option<Reaction>,
    reaction_started: Instant,
    walk_speed: f32,
    source_facing: Facing,
    facing: Facing,
    behavior: Behavior,
    mode_started: Instant,
    last_tick: Instant,
    frame_index: usize,
    subpixel: f32,
    walk_remaining: i32,
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
        let reaction_motion = |name| -> Result<Motion, String> {
            let assets = pack.resolve_motion(name)?;
            if matches!(name, "fall" | "land") && assets.frames == idle.frames {
                return Ok(Motion {
                    frames: vec![idle.frames[0].clone()],
                    fps: 4,
                    looped: false,
                });
            }
            Ok(Motion {
                frames: assets.frames.clone(),
                fps: assets.config.fps,
                looped: false,
            })
        };
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
            happy: reaction_motion("happy")?,
            special: reaction_motion("special")?,
            dragged: reaction_motion("dragged")?,
            fall: reaction_motion("fall")?,
            land: reaction_motion("land")?,
            reaction: None,
            reaction_started: now,
            walk_speed: pack.config.motion_or_default("walk").speed.unwrap_or(90.0),
            source_facing: pack.config.default_facing,
            facing: pack.config.default_facing,
            behavior: Behavior::new(now),
            mode_started: now,
            last_tick: now,
            frame_index: 0,
            subpixel: 0.0,
            walk_remaining: 0,
        })
    }

    fn motion(&self) -> &Motion {
        if let Some(reaction) = self.reaction {
            return match reaction {
                Reaction::Happy => &self.happy,
                Reaction::Special => &self.special,
                Reaction::Dragged => &self.dragged,
                Reaction::Fall => &self.fall,
                Reaction::Land => &self.land,
            };
        }
        match self.behavior.mode() {
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

    pub fn is_walking(&self) -> bool {
        self.reaction.is_none() && self.behavior.mode() == Mode::Walk
    }

    pub fn plan_walk(&mut self, left_room: i32, right_room: i32, now: Instant) -> bool {
        if !self.is_walking() {
            return false;
        }
        let left_room = left_room.max(0);
        let right_room = right_room.max(0);
        let preferred_room = if self.facing == Facing::Right {
            right_room
        } else {
            left_room
        };
        let opposite_room = if self.facing == Facing::Right {
            left_room
        } else {
            right_room
        };
        if preferred_room < self.walk_remaining.min(48) && opposite_room > preferred_room {
            self.reverse();
        }
        let available_room = if self.facing == Facing::Right {
            right_room
        } else {
            left_room
        };
        self.walk_remaining = self.walk_remaining.min(available_room);
        if self.walk_remaining == 0 {
            self.stop_walk(now);
            return true;
        }
        false
    }

    pub fn stop_walk(&mut self, now: Instant) {
        if !self.is_walking() {
            return;
        }
        self.behavior.finish_walk(now);
        self.mode_started = now;
        self.frame_index = 0;
        self.subpixel = 0.0;
        self.walk_remaining = 0;
        self.last_tick = now;
    }

    pub fn reset_idle(&mut self, now: Instant) -> bool {
        let redraw = self.frame() != self.idle.frames[0];
        self.reaction = None;
        self.behavior.reset_after_interaction(now);
        self.mode_started = now;
        self.last_tick = now;
        self.frame_index = 0;
        self.subpixel = 0.0;
        self.walk_remaining = 0;
        redraw
    }

    pub fn start_reaction(&mut self, reaction: Reaction, now: Instant) {
        self.behavior.reset_after_interaction(now);
        self.reaction = Some(reaction);
        self.reaction_started = now;
        self.mode_started = now;
        self.last_tick = now;
        self.frame_index = 0;
        self.subpixel = 0.0;
        self.walk_remaining = 0;
    }

    pub fn interval_ms(&self) -> u32 {
        if self.reaction.is_some() {
            return (1000 / u32::from(self.motion().fps)).max(1);
        }
        if self.behavior.mode() == Mode::Idle && self.idle.frames.len() == 1 {
            return self.behavior.decision_interval().as_millis() as u32;
        }
        let frame_ms = (1000 / u32::from(self.motion().fps)).max(1);
        if self.behavior.mode() == Mode::Walk {
            frame_ms.min(33)
        } else {
            frame_ms
        }
    }

    pub fn tick(&mut self, now: Instant) -> Tick {
        if let Some(reaction) = self.reaction {
            if reaction == Reaction::Dragged {
                return Tick {
                    dx: 0,
                    redraw: false,
                    mode_changed: false,
                };
            }
            let next_frame = (now.duration_since(self.reaction_started).as_millis()
                * u128::from(self.motion().fps)
                / 1000) as usize;
            if next_frame >= self.motion().frames.len() {
                if reaction == Reaction::Fall {
                    self.start_reaction(Reaction::Land, now);
                } else {
                    self.reset_idle(now);
                }
                return Tick {
                    dx: 0,
                    redraw: true,
                    mode_changed: true,
                };
            }
            let redraw = next_frame != self.frame_index;
            self.frame_index = next_frame;
            self.last_tick = now;
            return Tick {
                dx: 0,
                redraw,
                mode_changed: false,
            };
        }
        let mode_changed = self.behavior.tick(now);
        if mode_changed {
            self.mode_started = now;
            self.frame_index = 0;
            self.subpixel = 0.0;
            if self.behavior.mode() == Mode::Walk {
                self.facing = if self.behavior.walk_right() {
                    Facing::Right
                } else {
                    Facing::Left
                };
                self.walk_remaining = (self.walk_speed
                    * self.behavior.walk_duration().as_secs_f32())
                .round()
                .max(1.0) as i32;
            } else {
                self.walk_remaining = 0;
            }
        }

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
        if self.behavior.mode() == Mode::Walk && !mode_changed {
            let distance =
                self.walk_speed * now.duration_since(self.last_tick).as_secs_f32() + self.subpixel;
            let pixels = distance.floor() as i32;
            self.subpixel = distance - pixels as f32;
            let pixels = pixels.min(self.walk_remaining);
            self.walk_remaining -= pixels;
            dx = if self.facing == Facing::Right {
                pixels
            } else {
                -pixels
            };
            if self.walk_remaining == 0 {
                self.stop_walk(now);
                return Tick {
                    dx,
                    redraw: true,
                    mode_changed: true,
                };
            }
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

    fn test_motion(name: &str, fps: u16) -> Motion {
        Motion {
            frames: vec![name.into()],
            fps,
            looped: true,
        }
    }

    fn test_animation(now: Instant, seed: u64) -> Animation {
        Animation {
            idle: test_motion("idle", 4),
            walk: test_motion("walk", 8),
            happy: test_motion("happy", 6),
            special: test_motion("special", 6),
            dragged: test_motion("dragged", 1),
            fall: test_motion("fall", 4),
            land: test_motion("land", 4),
            reaction: None,
            reaction_started: now,
            walk_speed: 90.0,
            source_facing: Facing::Right,
            facing: Facing::Right,
            behavior: Behavior::with_seed(now, seed),
            mode_started: now,
            last_tick: now,
            frame_index: 0,
            subpixel: 0.0,
            walk_remaining: 0,
        }
    }

    #[test]
    fn frames_advance_and_loop_at_configured_fps() {
        assert_eq!(frame_index(Duration::from_millis(249), 4, 3, true), 0);
        assert_eq!(frame_index(Duration::from_millis(250), 4, 3, true), 1);
        assert_eq!(frame_index(Duration::from_millis(750), 4, 3, true), 0);
        assert_eq!(frame_index(Duration::from_millis(750), 4, 3, false), 2);
    }

    #[test]
    fn weighted_transition_still_moves_and_returns_to_idle() {
        let now = Instant::now();
        let seed = (1..10_000)
            .find(|seed| {
                let mut behavior = Behavior::with_seed(now, *seed);
                behavior.tick(now + Duration::from_millis(4500));
                behavior.mode() == Mode::Walk
                    && behavior.tick(now + Duration::from_millis(6500))
                    && behavior.mode() == Mode::Idle
            })
            .unwrap();
        let mut animation = test_animation(now, seed);
        assert_eq!(animation.interval_ms(), 4500);
        assert!(
            animation
                .tick(now + Duration::from_millis(4500))
                .mode_changed
        );
        assert_eq!(animation.frame(), Path::new("walk"));
        assert_ne!(animation.tick(now + Duration::from_millis(4600)).dx, 0);
        assert!(
            animation
                .tick(now + Duration::from_millis(6500))
                .mode_changed
        );
        assert_eq!(animation.frame(), Path::new("idle"));
    }

    #[test]
    fn each_walk_can_face_either_direction() {
        let now = Instant::now();
        for right in [false, true] {
            let seed = (1..10_000)
                .find(|seed| {
                    let mut behavior = Behavior::with_seed(now, *seed);
                    behavior.tick(now + Duration::from_millis(4500));
                    behavior.mode() == Mode::Walk && behavior.walk_right() == right
                })
                .unwrap();
            let mut animation = test_animation(now, seed);
            assert!(
                animation
                    .tick(now + Duration::from_millis(4500))
                    .mode_changed
            );
            assert_eq!(animation.should_flip(), !right);
            let dx = animation.tick(now + Duration::from_millis(4600)).dx;
            assert_eq!(dx.signum(), if right { 1 } else { -1 });
        }
    }

    #[test]
    fn walk_stops_at_its_chosen_distance_without_overshooting() {
        let now = Instant::now();
        let seed = (1..10_000)
            .find(|seed| {
                let mut behavior = Behavior::with_seed(now, *seed);
                behavior.tick(now + Duration::from_millis(4500));
                behavior.mode() == Mode::Walk
            })
            .unwrap();
        let mut animation = test_animation(now, seed);
        assert!(
            animation
                .tick(now + Duration::from_millis(4500))
                .mode_changed
        );
        let chosen_distance = animation.walk_remaining;
        assert!((63..=153).contains(&chosen_distance));

        let arrival = animation.tick(now + Duration::from_millis(6400));
        assert!(arrival.mode_changed);
        assert_eq!(arrival.dx.abs(), chosen_distance);
        assert_eq!(animation.frame(), Path::new("idle"));
        assert_eq!(animation.tick(now + Duration::from_millis(6500)).dx, 0);
    }

    #[test]
    fn walk_replays_its_distinct_sprite_frames_while_moving() {
        let now = Instant::now();
        let seed = (1..10_000)
            .find(|seed| {
                let mut behavior = Behavior::with_seed(now, *seed);
                behavior.tick(now + Duration::from_millis(4500));
                behavior.mode() == Mode::Walk
            })
            .unwrap();
        let mut animation = test_animation(now, seed);
        animation.walk = Motion {
            frames: (0..4).map(|index| format!("walk-{index}").into()).collect(),
            fps: 8,
            looped: true,
        };

        assert!(
            animation
                .tick(now + Duration::from_millis(4500))
                .mode_changed
        );
        assert_eq!(animation.frame(), Path::new("walk-0"));
        let first_step = animation.tick(now + Duration::from_millis(4625));
        assert!(first_step.dx != 0);
        assert!(first_step.redraw);
        assert_eq!(animation.frame(), Path::new("walk-1"));
        assert!(animation.tick(now + Duration::from_millis(4750)).redraw);
        assert_eq!(animation.frame(), Path::new("walk-2"));
    }

    #[test]
    fn walk_plans_away_from_a_nearby_edge() {
        let now = Instant::now();
        let seed = (1..10_000)
            .find(|seed| {
                let mut behavior = Behavior::with_seed(now, *seed);
                behavior.tick(now + Duration::from_millis(4500));
                behavior.mode() == Mode::Walk && behavior.walk_right()
            })
            .unwrap();
        let mut animation = test_animation(now, seed);
        let started = now + Duration::from_millis(4500);
        assert!(animation.tick(started).mode_changed);
        assert!(!animation.plan_walk(300, 4, started));
        assert!(animation.should_flip());
        assert!(animation.tick(started + Duration::from_millis(100)).dx < 0);
    }

    #[test]
    fn walk_without_available_room_returns_to_idle() {
        let now = Instant::now();
        let seed = (1..10_000)
            .find(|seed| {
                let mut behavior = Behavior::with_seed(now, *seed);
                behavior.tick(now + Duration::from_millis(4500));
                behavior.mode() == Mode::Walk
            })
            .unwrap();
        let mut animation = test_animation(now, seed);
        let started = now + Duration::from_millis(4500);
        assert!(animation.tick(started).mode_changed);
        assert!(animation.plan_walk(0, 0, started));
        assert!(!animation.is_walking());
        assert_eq!(animation.frame(), Path::new("idle"));
    }

    #[test]
    fn repeating_idle_decision_does_not_restart_its_frame_loop() {
        let now = Instant::now();
        let seed = (1..1000)
            .find(|seed| {
                let mut behavior = Behavior::with_seed(now, *seed);
                !behavior.tick(now + Duration::from_millis(4500)) && behavior.mode() == Mode::Idle
            })
            .unwrap();
        let mut animation = test_animation(now, seed);
        animation.idle = Motion {
            frames: vec!["idle-0".into(), "idle-1".into()],
            fps: 1,
            looped: true,
        };
        assert!(
            !animation
                .tick(now + Duration::from_millis(5500))
                .mode_changed
        );
        assert_eq!(animation.frame(), Path::new("idle-1"));
    }

    #[test]
    fn click_and_drag_reactions_complete_and_resume_autonomous_idle() {
        let now = Instant::now();
        let mut animation = test_animation(now, 1);
        animation.start_reaction(Reaction::Happy, now);
        assert_eq!(animation.frame(), Path::new("happy"));
        assert!(
            animation
                .tick(now + Duration::from_millis(167))
                .mode_changed
        );
        assert_eq!(animation.frame(), Path::new("idle"));

        let dragged_at = now + Duration::from_secs(1);
        animation.start_reaction(Reaction::Dragged, dragged_at);
        assert_eq!(animation.frame(), Path::new("dragged"));
        assert!(!animation.tick(dragged_at + Duration::from_secs(1)).redraw);

        let dropped_at = dragged_at + Duration::from_secs(2);
        animation.start_reaction(Reaction::Fall, dropped_at);
        assert_eq!(animation.frame(), Path::new("fall"));
        assert!(
            animation
                .tick(dropped_at + Duration::from_millis(250))
                .mode_changed
        );
        assert_eq!(animation.frame(), Path::new("land"));
        assert!(
            animation
                .tick(dropped_at + Duration::from_millis(500))
                .mode_changed
        );
        assert_eq!(animation.frame(), Path::new("idle"));
    }
}
