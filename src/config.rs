//! Configuration Helper

use config::{Config, File};
use std::fmt;
use std::path::Path;

/// Configuration Error
#[derive(Debug)]
pub enum Error {
    /// Key Value Splitting
    KeyValueSplitting {
        /// details
        detail: String,
    },

    /// Compilation Error
    Compilation {
        /// source
        source: config::ConfigError,
    },

    /// Invalid Path
    InvalidPath {
        /// details
        detail: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::KeyValueSplitting { detail } => write!(
                f,
                "Could not identify configuration key / value: {}",
                detail
            ),
            Error::Compilation { source } => write!(f, "Could not merge configuration: {}", source),
            Error::InvalidPath { detail } => {
                write!(f, "Invalid configuration path: {}", detail)
            }
        }
    }
}

/// merge configuration
pub fn merge_configuration<'a, D: AsRef<str>, P: Into<Option<&'a str>> + Clone>(
    root_dir: &Path,
    sub_dirs: &[D],
    profile: P,
    overrides: Vec<String>,
) -> Result<Config, Error> {
    let mut builder = sub_dirs
        .iter()
        .try_fold(Config::builder(), |mut builder, sub_dir| {
            let dir_path = root_dir.join(sub_dir.as_ref());

            // First we read the default configuration.
            let default_path = dir_path.join("default");

            let default_path = default_path.to_str().ok_or_else(|| Error::InvalidPath {
                detail: "Could not get a string representation of the default path".to_owned(),
            })?;

            log::debug!("Reading default configuration from: {}", default_path);

            builder = builder.add_source(File::with_name(default_path));

            if let Some(profile) = profile.clone().into().map(String::from) {
                let profile_path = dir_path.join(&profile);
                let profile_path = profile_path.to_str().ok_or_else(|| Error::InvalidPath {
                    detail: format!(
                        "Could not get a string representation of the profile {} path",
                        profile
                    ),
                })?;

                log::debug!("Reading {} configuration from: {}", profile, profile_path);

                builder = builder.add_source(File::with_name(profile_path).required(true));
            }
            Ok::<_, Error>(builder)
        })?;

    // Add command line overrides
    if !overrides.is_empty() {
        builder = builder.add_source(config_from_args(overrides)?)
    }

    builder
        .build()
        .map_err(|err| Error::Compilation { source: err })
}

// Create a new configuration source from a list of assignments key=value
fn config_from_args(args: impl IntoIterator<Item = String>) -> Result<Config, Error> {
    let builder = args.into_iter().fold(Config::builder(), |builder, arg| {
        builder.add_source(File::from_str(&arg, config::FileFormat::Toml))
    });
    builder
        .build()
        .map_err(|err| Error::Compilation { source: err })
}
