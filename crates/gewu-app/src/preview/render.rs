//! Turn a Typst source, or a PDF beside it, into SVG or PNG. Missing tools
//! mean no picture, not a failed review.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use crate::snapshot::QuotePreview;

pub(super) fn compile_file(path: &Path) -> Option<String> {
    if !typst_available() {
        return None;
    }
    let key = format!("file:{}", path.display());
    cached(&key, || run_typst(None, path))
}

/// Compile `source` as a file inside `project`, so `#import "/…" ` resolves
/// against that project. The example file is removed afterward.
pub(super) fn compile_project(project: &Path, source: &str) -> Option<String> {
    if !typst_available() {
        return None;
    }
    let key = format!("proj:{}:{source}", project.display());
    cached(&key, || {
        let dir = project.join(".gewu");
        fs::create_dir_all(&dir).ok()?;
        let input = dir.join(format!("preview-{}-{}.typ", std::process::id(), unique()));
        fs::write(&input, source).ok()?;
        let svg = run_typst(Some(project), &input);
        let _ = fs::remove_file(&input);
        svg
    })
}

pub(super) fn compile_source(source: &str) -> Option<String> {
    if !typst_available() {
        return None;
    }
    let key = format!("src:{source}");
    cached(&key, || {
        let dir = scratch_dir()?;
        let svg = (|| {
            let input = dir.join("example.typ");
            fs::write(&input, source).ok()?;
            run_typst(None, &input)
        })();
        let _ = fs::remove_dir_all(dir);
        svg
    })
}

pub(super) fn typst_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("typst")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })
}

pub(super) fn rasterize_pdf(path: &Path) -> Option<QuotePreview> {
    let dir = scratch_dir()?;
    let finished = wait_for(
        Command::new("pdftoppm")
            .args([
                "-png",
                "-f",
                "1",
                "-l",
                "1",
                "-scale-to",
                "900",
                "-singlefile",
            ])
            .arg(path)
            .arg(dir.join("page"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?,
    );
    let png = finished
        .then(|| fs::read(dir.join("page.png")).ok())
        .flatten();
    let _ = fs::remove_dir_all(dir);
    png.map(|bytes| super::preview(super::LABEL_RENDERED, "image/png", &base64(&bytes)))
}

pub(super) fn scratch_dir() -> Option<PathBuf> {
    let dir =
        std::env::temp_dir().join(format!("gewu-preview-{}-{}", std::process::id(), unique()));
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn unique() -> u64 {
    static TICK: AtomicU64 = AtomicU64::new(0);
    TICK.fetch_add(1, Ordering::Relaxed)
}

pub(super) use super::paint::draws;

pub(super) fn sanitize_svg(svg: &str) -> Option<String> {
    let start = svg.find("<svg")?;
    let svg = svg[start..].to_owned();
    let lower = svg.to_ascii_lowercase();
    (!lower.contains("<script") && !lower.contains("javascript:")).then(|| super::frame::fit(&svg))
}

pub(super) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let (a, b, c) = (
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        );
        let triple = (u32::from(a) << 16) | (u32::from(b) << 8) | u32::from(c);
        out.push(TABLE[((triple >> 18) & 63) as usize] as char);
        out.push(TABLE[((triple >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn run_typst(project: Option<&Path>, input: &Path) -> Option<String> {
    let dir = scratch_dir()?;
    let output = dir.join("out.svg");
    let mut command = Command::new("typst");
    command.arg("compile");
    if let Some(project) = project {
        command.arg("--root").arg(project);
    }
    let finished = wait_for(
        command
            .args(["--format", "svg"])
            .arg(input)
            .arg(&output)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?,
    );
    let svg = finished.then(|| fs::read_to_string(&output).ok()).flatten();
    let _ = fs::remove_dir_all(dir);
    svg.and_then(|svg| sanitize_svg(&svg))
}

fn wait_for(mut child: std::process::Child) -> bool {
    let started = Instant::now();
    loop {
        if child.try_wait().ok().flatten().is_some() {
            return true;
        }
        if started.elapsed() > Duration::from_secs(4) {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn cached(key: &str, compute: impl FnOnce() -> Option<String>) -> Option<String> {
    static CACHE: Mutex<Option<HashMap<String, Option<String>>>> = Mutex::new(None);
    let mut guard = CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(hit) = cache.get(key) {
        return hit.clone();
    }
    drop(guard);
    let value = compute();
    CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(key.to_owned(), value.clone());
    value
}
