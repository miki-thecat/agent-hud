use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

pub const CONFIG_FILE_NAME: &str = "agent-hud.json";
pub const DEFAULT_WINDOW_WIDTH: u32 = 620;
pub const DEFAULT_WINDOW_HEIGHT: u32 = 720;
const MIN_WINDOW_WIDTH: u32 = 320;
const MAX_WINDOW_WIDTH: u32 = 3840;
const MIN_WINDOW_HEIGHT: u32 = 240;
const MAX_WINDOW_HEIGHT: u32 = 2160;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub window_width: u32,
    pub window_height: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
        }
    }
}

impl Config {
    pub fn load_optional(path: &Path) -> Result<Self, ConfigError> {
        match fs::read_to_string(path) {
            Ok(contents) => Self::from_json(&contents),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    pub fn from_json(contents: &str) -> Result<Self, ConfigError> {
        let value: serde_json::Value =
            serde_json::from_str(contents).map_err(|source| ConfigError::Parse { source })?;
        let object = value.as_object().ok_or(ConfigError::RootMustBeObject)?;
        let config = Self {
            window_width: read_u32(object, "window_width", DEFAULT_WINDOW_WIDTH)?,
            window_height: read_u32(object, "window_height", DEFAULT_WINDOW_HEIGHT)?,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        validate_range(
            "window_width",
            self.window_width,
            MIN_WINDOW_WIDTH,
            MAX_WINDOW_WIDTH,
        )?;
        validate_range(
            "window_height",
            self.window_height,
            MIN_WINDOW_HEIGHT,
            MAX_WINDOW_HEIGHT,
        )
    }
}

fn read_u32(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &'static str,
    default: u32,
) -> Result<u32, ConfigError> {
    let Some(value) = object.get(key) else {
        return Ok(default);
    };
    let number = value
        .as_u64()
        .ok_or(ConfigError::FieldMustBeUnsignedInteger { key })?;
    u32::try_from(number).map_err(|_| ConfigError::FieldMustBeUnsignedInteger { key })
}

fn validate_range(key: &'static str, value: u32, min: u32, max: u32) -> Result<(), ConfigError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(ConfigError::FieldOutOfRange {
            key,
            min,
            max,
            value,
        })
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        source: serde_json::Error,
    },
    RootMustBeObject,
    FieldMustBeUnsignedInteger {
        key: &'static str,
    },
    FieldOutOfRange {
        key: &'static str,
        min: u32,
        max: u32,
        value: u32,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "unable to read {}: {source}", path.display()),
            Self::Parse { source } => write!(f, "invalid JSON: {source}"),
            Self::RootMustBeObject => f.write_str("configuration root must be an object"),
            Self::FieldMustBeUnsignedInteger { key } => {
                write!(f, "{key} must be an unsigned integer")
            }
            Self::FieldOutOfRange {
                key,
                min,
                max,
                value,
            } => write!(f, "{key}={value} is outside {min}..={max}"),
        }
    }
}
impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::{Config, DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH};
    use std::{fs, path::PathBuf};
    #[test]
    fn defaults_are_explicit_and_stable() {
        assert_eq!(Config::default().window_width, DEFAULT_WINDOW_WIDTH);
        assert_eq!(Config::default().window_height, DEFAULT_WINDOW_HEIGHT);
    }
    #[test]
    fn missing_local_config_uses_defaults() {
        assert_eq!(
            Config::load_optional(&path("missing")).unwrap(),
            Config::default()
        );
    }
    #[test]
    fn valid_local_config_overrides_supported_values() {
        let c =
            Config::from_json(r#"{"window_width":800,"window_height":600,"future":true}"#).unwrap();
        assert_eq!((c.window_width, c.window_height), (800, 600));
    }
    #[test]
    fn malformed_and_invalid_values_are_rejected() {
        assert!(Config::from_json("not json").is_err());
        assert!(Config::from_json(r#"{"window_width":100}"#).is_err());
        assert!(Config::from_json(r#"{"window_height":"720"}"#).is_err());
        assert!(Config::from_json("[]").is_err());
    }
    #[test]
    fn local_config_file_is_loaded_deterministically() {
        let p = path("loaded");
        fs::write(&p, r#"{"window_width":900}"#).unwrap();
        assert_eq!(Config::load_optional(&p).unwrap().window_width, 900);
        fs::remove_file(p).unwrap();
    }
    fn path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "agent-hud-config-{label}-{}.json",
            std::process::id()
        ))
    }
}
