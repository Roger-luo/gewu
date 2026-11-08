//! Ensure `generate_context!` can resolve `frontendDist` during `cargo check`
//! even when `apps/desktop/dist` has not been built (and without touching that
//! directory, which the UI harness may own concurrently).
//!
//! When the real dist is missing, set `TAURI_CONFIG` so both `tauri-build` and
//! `generate_context!` embed the committed `ui-placeholder/` instead.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let real_dist = manifest_dir.join("../../apps/desktop/dist");
    let placeholder = manifest_dir.join("ui-placeholder");

    println!(
        "cargo:rerun-if-changed={}",
        real_dist.join("index.html").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        placeholder.join("index.html").display()
    );
    println!("cargo:rerun-if-changed=tauri.conf.json");

    if !real_dist.join("index.html").is_file() {
        assert!(
            placeholder.join("index.html").is_file(),
            "ui-placeholder/index.html must exist when apps/desktop/dist is missing"
        );
        let frontend = path_for_config(&placeholder);
        let override_json = format!(r#"{{"build":{{"frontendDist":"{frontend}"}}}}"#);
        // Visible to tauri-build in this process and to generate_context! later.
        unsafe {
            // SAFETY: build scripts are single-threaded; tauri reads this env.
            env::set_var("TAURI_CONFIG", &override_json);
        }
        println!("cargo:rustc-env=TAURI_CONFIG={override_json}");
    }

    tauri_build::build()
}

fn path_for_config(path: &Path) -> String {
    let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    absolute.to_string_lossy().replace('\\', "/")
}
