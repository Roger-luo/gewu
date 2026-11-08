use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{Error, Result};

pub const INPUT_PATH: &str = ".gewu/review-input.json";
pub const SPEC_PATH: &str = ".gewu/review-spec.json";
pub const STATE_ROOT: &str = ".gewu/harness";
pub const REVIEWS_ROOT: &str = ".gewu/reviews";

pub fn read<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let contents = fs::read(path)
        .map_err(|error| Error::message(format!("could not read {}: {error}", path.display())))?;
    serde_json::from_slice(&contents)
        .map_err(|error| Error::message(format!("invalid JSON in {}: {error}", path.display())))
}

pub fn write<T: serde::Serialize>(path: impl AsRef<Path>, value: &T) -> Result<()> {
    let path = path.as_ref();
    let parent = path
        .parent()
        .ok_or_else(|| Error::message("harness artifact path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = temporary_path(path);
    fs::write(&temporary, serde_json::to_vec_pretty(value)?).map_err(|error| {
        Error::message(format!("could not write {}: {error}", temporary.display()))
    })?;
    fs::rename(&temporary, path)
        .map_err(|error| Error::message(format!("could not replace {}: {error}", path.display())))
}

pub fn remove(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            Error::message(format!("could not remove {}: {error}", path.display()))
        })?;
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}
