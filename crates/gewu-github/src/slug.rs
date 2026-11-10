use std::fmt;

/// `owner/name` of a repository on github.com.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Slug {
    pub owner: String,
    pub name: String,
}

impl Slug {
    /// Reads a Git remote URL. Returns `None` for remotes that are not on
    /// github.com, which can be previewed but not published to.
    #[must_use]
    pub fn from_remote_url(url: &str) -> Option<Self> {
        let url = url.trim();
        let rest = [
            "https://github.com/",
            "http://github.com/",
            "ssh://git@github.com/",
            "git@github.com:",
        ]
        .iter()
        .find_map(|prefix| url.strip_prefix(prefix))?;
        let rest = rest.trim_end_matches('/');
        let rest = rest.strip_suffix(".git").unwrap_or(rest);
        let (owner, name) = rest.split_once('/')?;
        let valid = |part: &str| !part.is_empty() && !part.contains('/');
        (valid(owner) && valid(name)).then(|| Self {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }

    /// Reads `owner/name`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (owner, name) = text.split_once('/')?;
        (!owner.is_empty() && !name.is_empty() && !name.contains('/')).then(|| Self {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::Slug;

    #[test]
    fn reads_the_remote_forms_git_prints() {
        for url in [
            "https://github.com/Roger-luo/gewu.git",
            "https://github.com/Roger-luo/gewu",
            "git@github.com:Roger-luo/gewu.git",
            "ssh://git@github.com/Roger-luo/gewu.git",
        ] {
            let slug = Slug::from_remote_url(url).expect(url);
            assert_eq!(slug.to_string(), "Roger-luo/gewu");
        }
    }

    #[test]
    fn other_hosts_are_not_github() {
        assert!(Slug::from_remote_url("https://gitlab.com/a/b.git").is_none());
        assert!(Slug::from_remote_url("/srv/git/project").is_none());
        assert!(Slug::from_remote_url("https://github.com/only-owner").is_none());
    }

    #[test]
    fn parse_needs_an_owner_and_a_name() {
        assert_eq!(Slug::parse("acme/demo").expect("slug").name, "demo");
        assert!(Slug::parse("demo").is_none());
    }
}
