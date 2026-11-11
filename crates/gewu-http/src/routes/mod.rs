mod workbench;

use std::sync::Mutex;

use gewu_app::{Action, ReviewWorkspace};
use gewu_workbench::{GitHub, Home, Recorder, Workbench};
use serde::Serialize;
use tiny_http::{Method, Request};

use crate::{
    HttpError, Mode,
    response::{Json, Reply},
    static_files::StaticRoot,
};

/// The state requests act on.
pub(crate) enum Backend {
    Review(Box<Mutex<ReviewWorkspace>>),
    /// The HTTP server is the UI sandbox, so GitHub is always the
    /// [`Recorder`]: publishing from it never leaves the machine.
    Workbench(Workbench, Box<GitHub>),
}

impl Backend {
    pub(crate) fn open(mode: Mode) -> Result<Self, HttpError> {
        Ok(match mode {
            Mode::Review { root } => {
                Self::Review(Box::new(Mutex::new(ReviewWorkspace::open(&root)?)))
            }
            Mode::Workbench { home, gewu } => Self::Workbench(
                Workbench::open(Home::new(home), &gewu)?,
                Box::new(GitHub::recorded(Recorder::default())),
            ),
        })
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Review(_) => "review",
            Self::Workbench(..) => "workbench",
        }
    }
}

/// Tells the UI which screen to start on.
#[derive(Serialize)]
struct Health {
    ok: bool,
    mode: &'static str,
}

pub(crate) struct Routes {
    backend: Backend,
    static_files: StaticRoot,
}

impl Routes {
    pub(crate) fn new(backend: Backend, static_files: StaticRoot) -> Self {
        Self {
            backend,
            static_files,
        }
    }

    pub(crate) fn handle(&self, mut request: Request) {
        let url = request.url().to_owned();
        let (path, query) = url.split_once('?').unwrap_or((&url, ""));
        let method = request.method().clone();
        let reply = match (&method, path) {
            (Method::Get, "/api/health") => Json::ok(Health {
                ok: true,
                mode: self.backend.name(),
            }),
            (_, api) if api.starts_with("/api/") => match &self.backend {
                Backend::Review(workspace) => Self::review(workspace, &method, api, &mut request),
                Backend::Workbench(workbench, github) => {
                    workbench::route(workbench, github, &method, api, query, &mut request)
                }
            },
            (Method::Get | Method::Head, _) => self.static_files.response(path),
            _ => Json::error(404, "not found"),
        };
        let _ = request.respond(reply);
    }

    fn review(
        workspace: &Mutex<ReviewWorkspace>,
        method: &Method,
        path: &str,
        request: &mut Request,
    ) -> Reply {
        let Ok(mut workspace) = workspace.lock() else {
            return Json::error(500, "the review workspace is unavailable");
        };
        match (method, path) {
            (Method::Get, "/api/snapshot") => Json::ok(workspace.snapshot()),
            (Method::Post, "/api/dispatch") => match Json::body::<Action>(request) {
                Ok(action) => match workspace.dispatch(action) {
                    Ok(snapshot) => Json::ok(snapshot),
                    Err(error) => Json::error(400, &error.to_string()),
                },
                Err(reply) => reply,
            },
            _ => Json::error(404, "not found"),
        }
    }
}
