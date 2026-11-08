use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
};

use gewu_agent::Preset;

/// What the desktop shell opens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Launch {
    /// One prepared review, from `--root <dir>` or `GEWU_ROOT`.
    Review(PathBuf),
    /// The workbench, when no review root is given.
    Workbench,
}

impl Launch {
    /// `--root <dir>` or `--root=<dir>`, else a non-empty `root_env`, else
    /// the workbench.
    pub fn resolve(
        args: impl IntoIterator<Item = impl AsRef<str>>,
        root_env: Option<&str>,
    ) -> Self {
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let arg = arg.as_ref();
            if let Some(value) = arg.strip_prefix("--root=") {
                return Self::Review(PathBuf::from(value));
            }
            if arg == "--root"
                && let Some(value) = args.next()
            {
                return Self::Review(PathBuf::from(value.as_ref()));
            }
        }
        match root_env {
            Some(value) if !value.is_empty() => Self::Review(PathBuf::from(value)),
            _ => Self::Workbench,
        }
    }

    /// The `gewu` binary agents are told to run: `GEWU_BIN`, else one beside
    /// this executable (as bundled), else the first on the search path.
    pub fn gewu(exe: &Path, bin_env: Option<&OsStr>) -> Option<PathBuf> {
        if let Some(bin) = bin_env.filter(|bin| !bin.is_empty()) {
            return Some(PathBuf::from(bin));
        }
        let sibling = exe.with_file_name("gewu");
        if sibling.is_file() {
            return Some(sibling);
        }
        Preset::which("gewu")
    }

    pub fn from_env() -> Self {
        Self::resolve(env::args(), env::var("GEWU_ROOT").ok().as_deref())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::Launch;

    #[test]
    fn prefers_root_flag_over_env() {
        let launch = Launch::resolve(["gewu-desktop", "--root", "/from-flag"], Some("/from-env"));
        assert_eq!(launch, Launch::Review(PathBuf::from("/from-flag")));
    }

    #[test]
    fn accepts_equals_form_of_root_flag() {
        let launch = Launch::resolve(["gewu-desktop", "--root=/equals"], None);
        assert_eq!(launch, Launch::Review(PathBuf::from("/equals")));
    }

    #[test]
    fn falls_back_to_gewu_root_env() {
        let launch = Launch::resolve(["gewu-desktop"], Some("/from-env"));
        assert_eq!(launch, Launch::Review(PathBuf::from("/from-env")));
    }

    #[test]
    fn opens_the_workbench_without_a_root() {
        assert_eq!(Launch::resolve(["gewu-desktop"], None), Launch::Workbench);
        assert_eq!(
            Launch::resolve(["gewu-desktop"], Some("")),
            Launch::Workbench
        );
    }

    #[test]
    fn finds_gewu_beside_the_executable() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let gewu = directory.path().join("gewu");
        std::fs::write(&gewu, "").expect("write");
        let exe = directory.path().join("gewu-desktop");
        assert_eq!(Launch::gewu(&exe, None), Some(gewu));
        let named = Launch::gewu(&exe, Some("/opt/gewu".as_ref()));
        assert_eq!(named.as_deref(), Some(Path::new("/opt/gewu")));
    }
}
