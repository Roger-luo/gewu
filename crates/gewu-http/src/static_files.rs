use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use tiny_http::{Header, Response, StatusCode};

use crate::HttpError;

/// Serve files under `root`, falling back to `index.html` for missing paths
/// (SPA routing). Rejects path traversal.
pub(crate) struct StaticRoot {
    root: PathBuf,
}

impl StaticRoot {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Result<Self, HttpError> {
        let root = root.into();
        if !root.is_dir() {
            return Err(HttpError::BadStatic(root));
        }
        Ok(Self {
            root: fs::canonicalize(&root).unwrap_or(root),
        })
    }

    pub(crate) fn response(&self, url_path: &str) -> Response<std::io::Cursor<Vec<u8>>> {
        match self.resolve(url_path) {
            Some((path, content_type)) => match fs::read(&path) {
                Ok(bytes) => with_close(
                    Response::from_data(bytes)
                        .with_header(header("Content-Type", content_type))
                        .with_status_code(StatusCode(200)),
                ),
                Err(_) => self.index_or_not_found(),
            },
            None => self.index_or_not_found(),
        }
    }

    fn resolve(&self, url_path: &str) -> Option<(PathBuf, &'static str)> {
        let relative = url_path.trim_start_matches('/');
        if relative.is_empty() {
            return self.file("index.html");
        }
        let candidate = Path::new(relative);
        if candidate
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::RootDir))
        {
            return None;
        }
        let path = self.root.join(candidate);
        let path = fs::canonicalize(&path).ok()?;
        if !path.starts_with(&self.root) || !path.is_file() {
            return None;
        }
        let content_type = content_type_for(&path);
        Some((path, content_type))
    }

    fn file(&self, name: &str) -> Option<(PathBuf, &'static str)> {
        let path = self.root.join(name);
        path.is_file()
            .then(|| (path.clone(), content_type_for(&path)))
    }

    fn index_or_not_found(&self) -> Response<std::io::Cursor<Vec<u8>>> {
        match self
            .file("index.html")
            .and_then(|(path, content_type)| fs::read(path).ok().map(|bytes| (bytes, content_type)))
        {
            Some((bytes, content_type)) => with_close(
                Response::from_data(bytes)
                    .with_header(header("Content-Type", content_type))
                    .with_status_code(StatusCode(200)),
            ),
            None => with_close(
                Response::from_string("not found")
                    .with_status_code(StatusCode(404))
                    .with_header(header("Content-Type", "text/plain; charset=utf-8")),
            ),
        }
    }
}

fn with_close(response: Response<std::io::Cursor<Vec<u8>>>) -> Response<std::io::Cursor<Vec<u8>>> {
    response.with_header(header("Connection", "close"))
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "map" => "application/json; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("header bytes are ASCII")
}
