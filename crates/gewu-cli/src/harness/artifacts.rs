use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

pub const INPUT_PATH: &str = ".gewu/review-input.json";
pub const SPEC_PATH: &str = ".gewu/review-spec.json";
pub const STATE_ROOT: &str = ".gewu/harness";
pub const REVIEWS_ROOT: &str = ".gewu/reviews";

pub fn read<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let contents = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    serde_json::from_slice(&contents).with_context(|| format!("invalid JSON in {}", path.display()))
}

pub fn write<T: serde::Serialize>(path: impl AsRef<Path>, value: &T) -> Result<()> {
    let path = path.as_ref();
    let parent = path
        .parent()
        .context("harness artifact path has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = temporary_path(path);
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("could not write {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("could not replace {}", path.display()))
}

pub fn remove(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("could not remove {}", path.display()))?;
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".tmp");
    PathBuf::from(name)
}
