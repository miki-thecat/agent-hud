use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectIdentity {
    pub normalized_name: String,
    pub root_path: Option<PathBuf>,
    pub repository_identity: Option<String>,
}

impl ProjectIdentity {
    pub fn from_cwd(cwd: Option<&str>) -> Option<Self> {
        let cwd = cwd?.trim();
        if cwd.is_empty() {
            return None;
        }
        let path = PathBuf::from(cwd);
        if let Some(identity) = repository_identity(&path) {
            return Some(identity);
        }
        from_path(&path, None)
    }
}

fn repository_identity(cwd: &Path) -> Option<ProjectIdentity> {
    if !cwd.is_dir() {
        return None;
    }
    let output = Command::new("git")
        .args([
            "-C",
            cwd.to_str()?,
            "rev-parse",
            "--show-toplevel",
            "--git-common-dir",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let mut lines = stdout.lines();
    let root = PathBuf::from(lines.next()?.trim());
    let git_common_dir = PathBuf::from(lines.next()?.trim());
    let root = if root.is_absolute() {
        root
    } else {
        cwd.join(root)
    };
    let git_common_dir = if git_common_dir.is_absolute() {
        git_common_dir
    } else {
        cwd.join(git_common_dir)
    };
    let root = normalize_existing_path(root)?;
    let git_common_dir = normalize_existing_path(git_common_dir)?;
    from_path(&root, Some(git_common_dir.to_string_lossy().into_owned()))
}

fn from_path(path: &Path, repository_identity: Option<String>) -> Option<ProjectIdentity> {
    let normalized_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")?
        .to_owned();
    Some(ProjectIdentity {
        normalized_name,
        root_path: normalize_existing_path(path.to_owned()),
        repository_identity,
    })
}

fn normalize_existing_path(path: PathBuf) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

#[cfg(test)]
mod tests {
    use super::ProjectIdentity;
    #[test]
    fn repository_path_provides_root_and_repository_identity() {
        let identity = ProjectIdentity::from_cwd(Some(env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert_eq!(identity.normalized_name, "agent-hud");
        assert!(identity.root_path.is_some());
        assert!(identity.repository_identity.is_some());
    }

    #[test]
    fn nonexistent_path_falls_back_to_cwd_name() {
        let identity = ProjectIdentity::from_cwd(Some(r"C:\work\sample-project")).unwrap();
        assert_eq!(identity.normalized_name, "sample-project");
        assert_eq!(identity.root_path, None);
        assert_eq!(identity.repository_identity, None);
    }

    #[test]
    fn missing_project_information_is_none() {
        assert_eq!(ProjectIdentity::from_cwd(None), None);
        assert_eq!(ProjectIdentity::from_cwd(Some("   ")), None);
        assert!(ProjectIdentity::from_cwd(Some(r"C:\")).is_none());
    }
}
