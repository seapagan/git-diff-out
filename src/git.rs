use std::{
    collections::HashSet,
    ffi::OsStr,
    path::Path,
    process::{Command, Output, Stdio},
};

use crate::cli::Mode;

pub fn diff_args(mode: &Mode, base: Option<&str>) -> Vec<String> {
    let mut args = vec!["diff".into(), "--no-color".into()];
    match mode {
        Mode::Default | Mode::Unstaged => {}
        Mode::Staged => args.push("--staged".into()),
        Mode::All => args.push("HEAD".into()),
        Mode::Branch(_) => args.push(format!(
            "{}...HEAD",
            base.expect("branch mode needs a base")
        )),
        Mode::Commits(count) => args.push(format!("HEAD~{count}..HEAD")),
    }
    args
}

pub fn resolved_diff_args(
    mode: &Mode,
    base: Option<&str>,
    cwd: &Path,
    git_program: &OsStr,
) -> Result<Vec<String>, String> {
    let mut args = diff_args(mode, base);
    if !matches!(mode, Mode::All) {
        return Ok(args);
    }

    let head_exists = Command::new(git_program)
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .current_dir(cwd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to start git: {error}"))?
        .success();
    if head_exists {
        return Ok(args);
    }

    let output = Command::new(git_program)
        .args(["hash-object", "-t", "tree", "--stdin"])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("failed to start git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot resolve Git's empty tree: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let empty_tree = String::from_utf8(output.stdout)
        .map_err(|_| "git returned a non-UTF-8 empty-tree object ID".to_owned())?;
    let empty_tree = empty_tree.trim();
    if empty_tree.is_empty() {
        return Err("git returned an empty empty-tree object ID".into());
    }
    *args.last_mut().expect("all mode has a revision argument") = empty_tree.into();
    Ok(args)
}

pub fn choose_remote(upstream_remote: Option<&str>, remotes: &[String]) -> Option<String> {
    if let Some(remote) = upstream_remote.filter(|remote| remotes.iter().any(|item| item == remote))
    {
        return Some(remote.into());
    }
    if remotes.iter().any(|remote| remote == "origin") {
        return Some("origin".into());
    }
    (remotes.len() == 1).then(|| remotes[0].clone())
}

pub fn choose_base(
    remote_default: Option<(&str, &str)>,
    local_branches: &HashSet<String>,
) -> Option<String> {
    if let Some((remote, tracking_ref)) = remote_default {
        let prefix = format!("{remote}/");
        if let Some(branch) = tracking_ref.strip_prefix(&prefix) {
            return Some(if local_branches.contains(branch) {
                branch.into()
            } else {
                tracking_ref.into()
            });
        }
    }
    ["main", "master"]
        .into_iter()
        .find(|branch| local_branches.contains(*branch))
        .map(str::to_owned)
}

pub fn detect_base(cwd: &Path, git_program: &OsStr) -> Result<String, String> {
    let remotes = capture(git_program, cwd, ["remote"])?
        .map(|value| value.lines().map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    let current_branch = capture(
        git_program,
        cwd,
        ["symbolic-ref", "--quiet", "--short", "HEAD"],
    )?;
    let upstream_remote = if let Some(branch) = current_branch {
        capture(
            git_program,
            cwd,
            [
                "for-each-ref",
                "--format=%(upstream:remotename)",
                &format!("refs/heads/{branch}"),
            ],
        )?
    } else {
        None
    };
    let remote = choose_remote(upstream_remote.as_deref(), &remotes);
    let remote_head = if let Some(remote) = remote.as_deref() {
        capture(
            git_program,
            cwd,
            [
                "symbolic-ref",
                "--quiet",
                "--short",
                &format!("refs/remotes/{remote}/HEAD"),
            ],
        )?
        .map(|head| (remote.to_owned(), head))
    } else {
        None
    };
    let local_branches = capture(
        git_program,
        cwd,
        ["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )?
    .map(|value| value.lines().map(str::to_owned).collect::<HashSet<_>>())
    .unwrap_or_default();

    choose_base(
        remote_head
            .as_ref()
            .map(|(remote, head)| (remote.as_str(), head.as_str())),
        &local_branches,
    )
    .ok_or_else(|| {
        "cannot determine a branch base; no remote default or local main/master branch found\n    \
         specify one with 'gd branch <BASE>' or configure base_branch"
            .into()
    })
}

pub fn repository_name(cwd: &Path, git_program: &OsStr) -> Result<String, String> {
    let candidates = repository_remote_candidates(cwd, git_program)?;
    if let Some(path) = repository_name_from_remotes(cwd, git_program, candidates)? {
        return Ok(path);
    }
    repository_root_name(cwd, git_program)
}

fn repository_remote_candidates(cwd: &Path, git_program: &OsStr) -> Result<Vec<String>, String> {
    let remotes = capture(git_program, cwd, ["remote"])?
        .map(|value| value.lines().map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    let current_branch = capture(
        git_program,
        cwd,
        ["symbolic-ref", "--quiet", "--short", "HEAD"],
    )?;
    let upstream = if let Some(branch) = current_branch {
        capture(
            git_program,
            cwd,
            [
                "for-each-ref",
                "--format=%(upstream:remotename)",
                &format!("refs/heads/{branch}"),
            ],
        )?
    } else {
        None
    };

    let mut candidates = Vec::new();
    if let Some(remote) = upstream {
        candidates.push(remote);
    }
    candidates.push("origin".into());
    candidates.extend(remotes);
    Ok(candidates)
}

fn repository_name_from_remotes(
    cwd: &Path,
    git_program: &OsStr,
    candidates: Vec<String>,
) -> Result<Option<String>, String> {
    let mut seen = HashSet::new();
    for remote in candidates {
        if !seen.insert(remote.clone()) {
            continue;
        }
        if let Some(path) =
            capture_remote_url(git_program, cwd, &remote)?.and_then(|url| remote_path(&url))
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn repository_root_name(cwd: &Path, git_program: &OsStr) -> Result<String, String> {
    let root = capture(git_program, cwd, ["rev-parse", "--show-toplevel"])?
        .ok_or_else(|| "cannot determine the Git repository root".to_owned())?;
    Path::new(&root)
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "cannot determine the Git repository name".to_owned())
}

fn remote_path(url: &str) -> Option<String> {
    if matches!(url.as_bytes(), [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic()) {
        return None;
    }
    let path = if let Some((_, address)) = url.split_once("://") {
        let (host, path) = address.split_once('/')?;
        if host.is_empty() {
            return None;
        }
        path.split(['?', '#']).next().unwrap_or(path)
    } else {
        let colon = url.rfind(':')?;
        if url[..colon].contains('/') || url[..colon].contains('\\') {
            return None;
        }
        &url[colon + 1..]
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    (!path.is_empty()).then(|| path.to_owned())
}

fn capture<const N: usize>(
    git_program: &OsStr,
    cwd: &Path,
    args: [&str; N],
) -> Result<Option<String>, String> {
    let output = Command::new(git_program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("failed to start git: {error}"))?;
    successful_stdout(output)
}

fn capture_remote_url(
    git_program: &OsStr,
    cwd: &Path,
    remote: &str,
) -> Result<Option<String>, String> {
    let output = Command::new(git_program)
        .args(["remote", "get-url", remote])
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("failed to start git: {error}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    let Ok(value) = String::from_utf8(output.stdout) else {
        return Ok(None);
    };
    let value = value.trim();
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

fn successful_stdout(output: Output) -> Result<Option<String>, String> {
    if !output.status.success() {
        return Ok(None);
    }
    let value = String::from_utf8(output.stdout)
        .map_err(|_| "git returned non-UTF-8 reference data".to_owned())?;
    let value = value.trim();
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::remote_path;

    #[test]
    fn windows_paths_are_not_remote_repository_paths() {
        for path in [
            r"C:\path\to\repo",
            "C:/path/to/repo",
            r"d:\work\repo.git",
            "d:/work/repo.git",
            r"\\server\share\repo",
            "//server/share/repo",
        ] {
            assert_eq!(remote_path(path), None, "path: {path}");
        }
    }

    #[test]
    fn git_remote_paths_remain_supported() {
        for (url, expected) in [
            ("x:group/project.git", "group/project"),
            ("a:repo.git", "repo"),
            ("git@github.com:owner/repo.git", "owner/repo"),
            ("git@example.com:group/project.git", "group/project"),
            ("ssh://git@example.com/group/project.git", "group/project"),
            ("https://example.com/group/project.git", "group/project"),
        ] {
            assert_eq!(remote_path(url).as_deref(), Some(expected), "url: {url}");
        }
    }

    #[test]
    fn scp_remote_paths_preserve_url_delimiter_characters() {
        for (url, expected) in [
            ("git@example.com:path/to/repo?name.git", "path/to/repo?name"),
            ("git@example.com:path/to/repo#name.git", "path/to/repo#name"),
        ] {
            assert_eq!(remote_path(url).as_deref(), Some(expected), "url: {url}");
        }
    }

    #[test]
    fn url_metadata_is_excluded_from_remote_repository_paths() {
        for (url, expected) in [
            (
                "https://example.com/owner/repo.git?access_token=TOPSECRET",
                "owner/repo",
            ),
            ("https://example.com/owner/repo.git#fragment", "owner/repo"),
            (
                "https://user:secret@example.com/owner/repo.git?token=abc#frag",
                "owner/repo",
            ),
        ] {
            assert_eq!(remote_path(url).as_deref(), Some(expected), "url: {url}");
        }
    }
}
