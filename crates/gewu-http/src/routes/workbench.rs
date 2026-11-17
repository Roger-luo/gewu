use gewu_app::Action;
use gewu_workbench::{
    GitHub, Preferences, PromptReport, PromptView, SignInMethod, Workbench, WorkbenchAction,
    WorkbenchError,
};
use serde::{Deserialize, Serialize};
use tiny_http::{Method, Request};

use crate::response::{Json, Reply};

#[derive(Deserialize)]
struct SignIn {
    token: String,
}

#[derive(Deserialize)]
struct UseMethod {
    method: SignInMethod,
}

/// `/api/workbench`, `/api/repositories/<id>/refs`, `/api/prompts`, `/api/tasks/<id>/…`,
/// and `/api/github/…`.
pub(super) fn route(
    workbench: &Workbench,
    github: &GitHub,
    method: &Method,
    path: &str,
    query: &str,
    request: &mut Request,
) -> Reply {
    let parts: Vec<&str> = path.trim_start_matches("/api/").split('/').collect();
    match (method, parts.as_slice()) {
        (Method::Get, ["workbench"]) => answer(workbench.snapshot()),
        (Method::Post, ["workbench", "dispatch"]) => match Json::body::<WorkbenchAction>(request) {
            Ok(action) => answer(workbench.dispatch(action)),
            Err(reply) => reply,
        },
        (Method::Get, ["repositories", id, "refs"]) => answer(workbench.refs(id)),
        (Method::Get, ["prompts"]) => Json::ok(PromptView::all()),
        (Method::Post, ["prompts", "report"]) => match Json::body::<PromptReport>(request) {
            Ok(report) => answer(
                github
                    .with_transport(|github| report.send(github))
                    .map_err(Into::into)
                    .and_then(|reply| reply),
            ),
            Err(reply) => reply,
        },
        (Method::Get, ["tasks", id, "log"]) => answer(workbench.log(id, after(query))),
        (Method::Get, ["tasks", id, "snapshot"]) => answer(workbench.task_snapshot(id)),
        (Method::Post, ["tasks", id, "dispatch"]) => match Json::body::<Action>(request) {
            Ok(action) => answer(workbench.task_dispatch(id, action)),
            Err(reply) => reply,
        },
        (Method::Get, ["tasks", id, "publication"]) => answer(workbench.publication(id)),
        (Method::Post, ["tasks", id, "publish"]) => answer(
            github
                .with_transport(|github| workbench.publish(id, github))
                .map_err(Into::into)
                .and_then(|reply| reply),
        ),
        (Method::Get, ["tasks", id, "contributors"]) => answer(
            github
                .with_transport(|github| workbench.contributors(id, github))
                .map_err(Into::into)
                .and_then(|reply| reply),
        ),
        (Method::Get, ["github", "account"]) => answer(github.account().map_err(Into::into)),
        (Method::Get, ["github", "settings"]) => answer(github.settings().map_err(Into::into)),
        (Method::Post, ["github", "settings"]) => match Json::body::<Preferences>(request) {
            Ok(preferences) => answer(github.set_preferences(preferences).map_err(Into::into)),
            Err(reply) => reply,
        },
        (Method::Post, ["github", "method"]) => match Json::body::<UseMethod>(request) {
            Ok(choice) => answer(github.use_method(choice.method).map_err(Into::into)),
            Err(reply) => reply,
        },
        (Method::Post, ["github", "sign-in"]) => match Json::body::<SignIn>(request) {
            Ok(sign_in) if sign_in.token.trim().is_empty() => {
                Json::error(400, "paste a GitHub token to sign in")
            }
            Ok(sign_in) => answer(
                github
                    .sign_in_with_token(&sign_in.token)
                    .map_err(Into::into),
            ),
            Err(reply) => reply,
        },
        (Method::Post, ["github", "sign-in", "start"]) => {
            answer(github.start().map_err(Into::into))
        }
        (Method::Post, ["github", "sign-in", "poll"]) => answer(github.poll().map_err(Into::into)),
        (Method::Post, ["github", "sign-out"]) => answer(github.sign_out().map_err(Into::into)),
        _ => Json::error(404, "not found"),
    }
}

fn answer<T: Serialize>(result: Result<T, WorkbenchError>) -> Reply {
    match result {
        Ok(value) => Json::ok(value),
        Err(error @ WorkbenchError::NotFound(_)) => Json::error(404, &error.to_string()),
        Err(error) if error.is_client_error() => Json::error(400, &error.to_string()),
        Err(error) => Json::error(500, &error.to_string()),
    }
}

/// The `after` query parameter, or `0`.
fn after(query: &str) -> usize {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("after="))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}
