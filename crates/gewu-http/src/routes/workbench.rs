use gewu_app::Action;
use gewu_workbench::{Account, Recorder, Workbench, WorkbenchAction, WorkbenchError};
use serde::{Deserialize, Serialize};
use tiny_http::{Method, Request};

use crate::response::{Json, Reply};

/// The sandbox's sign-in: any token signs in as this login.
const SANDBOX_LOGIN: &str = "gewu-sandbox";

#[derive(Deserialize)]
struct SignIn {
    token: String,
}

/// `/api/workbench`, `/api/repositories/<id>/refs`, `/api/tasks/<id>/…`,
/// and `/api/github/…`.
pub(super) fn route(
    workbench: &Workbench,
    github: &Recorder,
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
        (Method::Get, ["tasks", id, "log"]) => answer(workbench.log(id, after(query))),
        (Method::Get, ["tasks", id, "snapshot"]) => answer(workbench.task_snapshot(id)),
        (Method::Post, ["tasks", id, "dispatch"]) => match Json::body::<Action>(request) {
            Ok(action) => answer(workbench.task_dispatch(id, action)),
            Err(reply) => reply,
        },
        (Method::Get, ["tasks", id, "publication"]) => answer(workbench.publication(id)),
        (Method::Post, ["tasks", id, "publish"]) => answer(workbench.publish(id, github)),
        (Method::Get, ["github", "account"]) => {
            answer(Account::current(github).map_err(Into::into))
        }
        (Method::Post, ["github", "sign-in"]) => match Json::body::<SignIn>(request) {
            Ok(sign_in) if sign_in.token.trim().is_empty() => {
                Json::error(400, "paste a GitHub token to sign in")
            }
            Ok(_) => {
                github.set_login(Some(SANDBOX_LOGIN));
                answer(Account::current(github).map_err(Into::into))
            }
            Err(reply) => reply,
        },
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
