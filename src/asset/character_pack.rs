use std::{
    collections::{BTreeMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

use crate::config::character::{CharacterConfig, MOTION_NAMES, MotionConfig};

#[derive(Debug)]
pub struct MotionAssets {
    pub config: MotionConfig,
    pub frames: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct CharacterPack {
    pub id: String,
    pub root: PathBuf,
    pub config: CharacterConfig,
    motions: BTreeMap<String, MotionAssets>,
}

impl CharacterPack {
    pub fn load_first(root: &Path) -> Result<Self, String> {
        let mut candidates = fs::read_dir(root)
            .map_err(|error| format!("failed to read character root {}: {error}", root.display()))?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                !name.starts_with('_') && entry.path().join("character.toml").is_file()
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();

        candidates.sort_by_key(|path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        });

        let path = candidates.first().ok_or_else(|| {
            format!(
                "no runnable character pack found in {} (folders beginning with '_' are ignored)",
                root.display()
            )
        })?;

        Self::load(path)
    }

    pub fn load(root: &Path) -> Result<Self, String> {
        let config_path = root.join("character.toml");
        let config = CharacterConfig::from_path(&config_path)?;
        let id = root
            .file_name()
            .ok_or_else(|| format!("invalid character pack path: {}", root.display()))?
            .to_string_lossy()
            .into_owned();

        let mut motions = BTreeMap::new();

        for motion_name in MOTION_NAMES {
            let frames = collect_png_frames(&root.join(motion_name))?;
            if frames.is_empty() {
                continue;
            }

            motions.insert(
                motion_name.to_string(),
                MotionAssets {
                    config: config.motion_or_default(motion_name),
                    frames,
                },
            );
        }

        if !motions.contains_key("idle") {
            return Err(format!(
                "character pack '{}' must contain at least one PNG in {}/idle",
                id,
                root.display()
            ));
        }

        // Only inspect image headers here; pixel data is decoded when a frame is displayed.
        let first_idle = &motions["idle"].frames[0];
        let expected_size = image::image_dimensions(first_idle).map_err(|error| {
            format!(
                "failed to read dimensions of {}: {error}",
                first_idle.display()
            )
        })?;
        for (motion_name, motion) in &motions {
            for frame in &motion.frames {
                let size = image::image_dimensions(frame).map_err(|error| {
                    format!("failed to read dimensions of {}: {error}", frame.display())
                })?;
                if size != expected_size {
                    return Err(format!(
                        "frame {} in motion '{}' is {}x{}; expected {}x{} to match the first idle frame",
                        frame.display(),
                        motion_name,
                        size.0,
                        size.1,
                        expected_size.0,
                        expected_size.1
                    ));
                }
            }
        }

        Ok(Self {
            id,
            root: root.to_path_buf(),
            config,
            motions,
        })
    }

    pub fn resolve_motion(&self, requested: &str) -> Result<&MotionAssets, String> {
        if !MOTION_NAMES.contains(&requested) {
            return Err(format!("unknown motion '{requested}'"));
        }

        let mut current = requested;
        let mut visited = HashSet::new();

        loop {
            if !visited.insert(current.to_string()) {
                return Err(format!(
                    "fallback cycle detected while resolving motion '{requested}'"
                ));
            }

            if let Some(motion) = self.motions.get(current) {
                return Ok(motion);
            }

            if let Some(fallback) = self.config.fallbacks.get(current) {
                current = fallback;
                continue;
            }

            if current != "idle" {
                current = "idle";
                continue;
            }

            return Err(format!(
                "motion '{requested}' cannot resolve to an available animation"
            ));
        }
    }

    pub fn available_motion_names(&self) -> impl Iterator<Item = &str> {
        self.motions.keys().map(String::as_str)
    }
}

pub fn default_character_root() -> Result<PathBuf, String> {
    if let Some(explicit_root) = env::var_os("DESKTOP_PET_CHARACTER_ROOT") {
        let path = PathBuf::from(explicit_root);
        if path.is_dir() {
            return Ok(path);
        }

        return Err(format!(
            "DESKTOP_PET_CHARACTER_ROOT does not point to a directory: {}",
            path.display()
        ));
    }

    let mut candidates = Vec::new();

    if let Ok(executable) = env::current_exe()
        && let Some(parent) = executable.parent()
    {
        candidates.push(parent.join("assets").join("characters"));
    }

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("assets").join("characters"));
        candidates.push(
            current_dir
                .join("DesktopPet")
                .join("assets")
                .join("characters"),
        );
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("characters"),
    );

    for candidate in candidates {
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }

    Err("could not locate assets/characters directory".into())
}

fn collect_png_frames(directory: &Path) -> Result<Vec<PathBuf>, String> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }

    let mut frames = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        })
        .collect::<Vec<_>>();

    frames.sort_by_key(|path| {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });

    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("characters")
    }

    #[test]
    fn load_first_ignores_template_folder() {
        let pack = CharacterPack::load_first(&character_root()).unwrap();

        assert_eq!(pack.id, "default");
    }

    #[test]
    fn missing_walk_frames_fall_back_to_idle() {
        let pack = CharacterPack::load(&character_root().join("default")).unwrap();
        let idle = pack.resolve_motion("idle").unwrap();
        let walk = pack.resolve_motion("walk").unwrap();

        assert_eq!(walk.frames, idle.frames);
    }

    #[test]
    fn rejects_frame_size_mismatch_before_animation() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "desktop-pet-frame-size-{}-{unique}",
            std::process::id()
        ));
        let idle_dir = root.join("idle");
        let walk_dir = root.join("walk");
        fs::create_dir_all(&idle_dir).unwrap();
        fs::create_dir_all(&walk_dir).unwrap();
        let config = root.join("character.toml");
        fs::write(&config, "name = \"Size check\"\n").unwrap();
        let idle_frame = idle_dir.join("0001.png");
        let walk_frame = walk_dir.join("0001.png");
        image::RgbaImage::new(16, 16).save(&idle_frame).unwrap();
        image::RgbaImage::new(16, 16).save(&walk_frame).unwrap();
        assert!(CharacterPack::load(&root).is_ok());

        image::RgbaImage::new(32, 32).save(&walk_frame).unwrap();
        let error = CharacterPack::load(&root).unwrap_err();
        assert!(error.contains("expected 16x16"), "{error}");
        assert!(error.contains("walk"), "{error}");

        fs::remove_file(walk_frame).unwrap();
        fs::remove_file(idle_frame).unwrap();
        fs::remove_file(config).unwrap();
        fs::remove_dir(walk_dir).unwrap();
        fs::remove_dir(idle_dir).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
