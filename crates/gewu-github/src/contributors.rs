//! Who a comment can `@`-mention: the repository's contributors.

use serde::{Deserialize, Serialize};

use crate::{GitHubError, Slug, Transport};

/// One account that committed to the repository.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Contributor {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Deserialize)]
struct Listed {
    login: String,
    avatar_url: String,
    #[serde(default, rename = "type")]
    kind: String,
}

impl Contributor {
    /// The repository's top 100 human contributors, most commits first.
    /// Reading them notifies nobody. Signed out, the list is empty.
    pub fn list(transport: &dyn Transport, slug: &Slug) -> Result<Vec<Self>, GitHubError> {
        let path = format!("/repos/{slug}/contributors?per_page=100");
        let reply = match transport.call("GET", &path, None) {
            Ok(reply) => reply,
            Err(GitHubError::SignedOut) => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        // An empty repository answers 204 with no body.
        if reply.is_null() {
            return Ok(Vec::new());
        }
        let listed: Vec<Listed> =
            serde_json::from_value(reply).map_err(|error| GitHubError::Reply(error.to_string()))?;
        Ok(listed
            .into_iter()
            .filter(|listed| listed.kind != "Bot")
            .map(|listed| Self {
                login: listed.login,
                avatar_url: listed.avatar_url,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::Contributor;
    use crate::{Recorder, Slug};

    #[test]
    fn lists_people_and_leaves_out_bots() {
        let github = Recorder::signed_in("ada");
        let slug = Slug::parse("acme/demo").expect("slug");
        let logins: Vec<String> = Contributor::list(&github, &slug)
            .expect("contributors")
            .into_iter()
            .map(|contributor| contributor.login)
            .collect();
        assert_eq!(logins, ["ada", "grace", "linus"]);
        let (method, path, _) = github.calls().pop().expect("call");
        assert_eq!(
            (method.as_str(), path.as_str()),
            ("GET", "/repos/acme/demo/contributors?per_page=100")
        );
    }

    #[test]
    fn signed_out_lists_nobody() {
        let slug = Slug::parse("acme/demo").expect("slug");
        assert!(
            Contributor::list(&Recorder::default(), &slug)
                .expect("contributors")
                .is_empty()
        );
    }
}
