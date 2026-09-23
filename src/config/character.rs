use std::{collections::BTreeMap, fs, path::Path};

use serde::Deserialize;

pub const MOTION_NAMES: [&str; 12] = [
    "idle", "walk", "dragged", "happy", "look", "sleep", "wake", "sit", "stretch", "fall", "land",
    "special",
];

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Facing {
    Left,
    #[default]
    Right,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MotionConfig {
    #[serde(default = "default_fps")]
    pub fps: u16,
    #[serde(default = "default_loop", rename = "loop")]
    pub looped: bool,
    #[serde(default)]
    pub speed: Option<f32>,
}

impl MotionConfig {
    pub fn default_for(name: &str) -> Self {
        match name {
            "walk" => Self {
                fps: 8,
                looped: true,
                speed: Some(90.0),
            },
            "dragged" => Self {
                fps: 1,
                looped: true,
                speed: None,
            },
            "happy" | "look" | "wake" | "land" | "special" => Self {
                fps: 6,
                looped: false,
                speed: None,
            },
            _ => Self {
                fps: 4,
                looped: true,
                speed: None,
            },
        }
    }

    fn validate(&self, name: &str) -> Result<(), String> {
        if self.fps == 0 {
            return Err(format!("motion '{name}' fps must be greater than 0"));
        }

        if let Some(speed) = self.speed
            && (!speed.is_finite() || speed <= 0.0)
        {
            return Err(format!(
                "motion '{name}' speed must be a finite value greater than 0"
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CharacterConfig {
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_scale")]
    pub scale: f32,
    #[serde(default)]
    pub anchor_x: i32,
    #[serde(default)]
    pub anchor_y: i32,
    #[serde(default)]
    pub default_facing: Facing,

    #[serde(default)]
    pub idle: Option<MotionConfig>,
    #[serde(default)]
    pub walk: Option<MotionConfig>,
    #[serde(default)]
    pub dragged: Option<MotionConfig>,
    #[serde(default)]
    pub happy: Option<MotionConfig>,
    #[serde(default)]
    pub look: Option<MotionConfig>,
    #[serde(default)]
    pub sleep: Option<MotionConfig>,
    #[serde(default)]
    pub wake: Option<MotionConfig>,
    #[serde(default)]
    pub sit: Option<MotionConfig>,
    #[serde(default)]
    pub stretch: Option<MotionConfig>,
    #[serde(default)]
    pub fall: Option<MotionConfig>,
    #[serde(default)]
    pub land: Option<MotionConfig>,
    #[serde(default)]
    pub special: Option<MotionConfig>,

    #[serde(default)]
    pub fallbacks: BTreeMap<String, String>,
}

impl CharacterConfig {
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let config: Self = toml::from_str(&text)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    pub fn motion(&self, name: &str) -> Option<&MotionConfig> {
        match name {
            "idle" => self.idle.as_ref(),
            "walk" => self.walk.as_ref(),
            "dragged" => self.dragged.as_ref(),
            "happy" => self.happy.as_ref(),
            "look" => self.look.as_ref(),
            "sleep" => self.sleep.as_ref(),
            "wake" => self.wake.as_ref(),
            "sit" => self.sit.as_ref(),
            "stretch" => self.stretch.as_ref(),
            "fall" => self.fall.as_ref(),
            "land" => self.land.as_ref(),
            "special" => self.special.as_ref(),
            _ => None,
        }
    }

    pub fn motion_or_default(&self, name: &str) -> MotionConfig {
        self.motion(name)
            .cloned()
            .unwrap_or_else(|| MotionConfig::default_for(name))
    }

    fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("character name must not be empty".into());
        }

        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err("scale must be a finite value greater than 0".into());
        }

        for motion_name in MOTION_NAMES {
            if let Some(motion) = self.motion(motion_name) {
                motion.validate(motion_name)?;
            }
        }

        for (motion, fallback) in &self.fallbacks {
            if !MOTION_NAMES.contains(&motion.as_str()) {
                return Err(format!("unknown fallback source motion '{motion}'"));
            }
            if !MOTION_NAMES.contains(&fallback.as_str()) {
                return Err(format!("unknown fallback target motion '{fallback}'"));
            }
        }

        Ok(())
    }
}

fn default_fps() -> u16 {
    4
}

fn default_loop() -> bool {
    true
}

fn default_scale() -> f32 {
    1.0
}

fn default_version() -> String {
    "0.1.0".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_config_uses_defaults() {
        let config: CharacterConfig = toml::from_str("name = \"Test Pet\"").unwrap();

        assert_eq!(config.name, "Test Pet");
        assert_eq!(config.scale, 1.0);
        assert!(matches!(config.default_facing, Facing::Right));
        assert_eq!(config.motion_or_default("walk").fps, 8);
        assert_eq!(config.motion_or_default("walk").speed, Some(90.0));
    }

    #[test]
    fn configured_motion_keeps_values() {
        let config: CharacterConfig = toml::from_str(
            r#"
name = "Test Pet"

[walk]
fps = 10
loop = false
speed = 72.5
"#,
        )
        .unwrap();

        let walk = config.motion_or_default("walk");
        assert_eq!(walk.fps, 10);
        assert!(!walk.looped);
        assert_eq!(walk.speed, Some(72.5));
    }
}
