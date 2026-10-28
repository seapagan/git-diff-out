use std::{
    collections::HashSet,
    error::Error,
    ffi::{OsStr, OsString},
    fs, io,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use crate::{cli::Mode, git::resolved_diff_args};

mod artifacts;

pub(crate) fn render(
    cwd: &Path,
    git: &OsStr,
    output_dir: &Path,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let (_temporary, mut command) = command(cwd, git, output_dir)?;
    Ok(checked(&mut command)?.stdout)
}

pub(crate) fn command(
    cwd: &Path,
    git: &OsStr,
    output_dir: &Path,
) -> Result<(tempfile::TempDir, Command), Box<dyn Error>> {
    let root = repository_root(git, cwd)?;
    let args = resolved_diff_args(&Mode::All, None, cwd, git)?;
    let paths = untracked_paths(
        git,
        cwd,
        &root,
        output_dir,
        args.last().expect("all mode has a revision"),
    )?;
    let temporary = tempfile::tempdir()
        .map_err(|error| format!("cannot create temporary Git index directory: {error}"))?;
    let index = temporary.path().join("index");
    prepare_index(git, cwd, &index, &paths)?;
    // Keep the temporary index full to avoid Git 2.55 sparse-index expansion crashes.
    let mut command = Command::new(git);
    command
        .current_dir(cwd)
        .env("GIT_INDEX_FILE", &index)
        .args(["-c", "index.sparse=false"])
        .args(args);
    Ok((temporary, command))
}

fn repository_root(git: &OsStr, cwd: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let mut command = Command::new(git);
    command.current_dir(cwd);
    let prefix = checked(command.args(["rev-parse", "--show-cdup"]))?;
    Ok(cwd.join(path_from_bytes(trim_newline(prefix.stdout))?))
}

fn untracked_paths(
    git: &OsStr,
    cwd: &Path,
    root: &Path,
    output_dir: &Path,
    revision: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut command = Command::new(git);
    // Keep inherited relative Git paths anchored to the invocation directory.
    command.current_dir(cwd);
    let candidates = checked(
        command
            .args([
                "ls-files",
                "--others",
                "--exclude-standard",
                "--full-name",
                "-z",
                "--",
            ])
            .arg(root),
    )?;
    let mut command = Command::new(git);
    command
        .current_dir(cwd)
        .args(["ls-tree", "--full-tree", "-r", "--name-only", "-z"])
        .arg(revision);
    let tracked = checked(&mut command)?.stdout;
    selected_paths(root, output_dir, &candidates.stdout, &tracked)
}

fn prepare_index(
    git: &OsStr,
    cwd: &Path,
    index: &Path,
    paths: &[u8],
) -> Result<(), Box<dyn Error>> {
    copy_index(git, cwd, index)?;
    if !index.exists() {
        let mut command = Command::new(git);
        command.current_dir(cwd).env("GIT_INDEX_FILE", index).args([
            "-c",
            "core.splitIndex=false",
            "read-tree",
            "--empty",
        ]);
        checked(&mut command)?;
    }
    if !paths.is_empty() {
        add_intent(git, cwd, index, paths)?;
    }
    Ok(())
}

fn copy_index(git: &OsStr, cwd: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new(git);
    command.current_dir(cwd);
    let output =
        checked(command.args(["rev-parse", "--path-format=absolute", "--git-path", "index"]))?;
    let source = path_from_bytes(trim_newline(output.stdout))?;
    let mut source = match fs::File::open(source) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("cannot copy Git index: {error}").into()),
    };
    let modified = source.metadata()?.modified()?;
    let mut copy = fs::File::create(destination)?;
    io::copy(&mut source, &mut copy)?;
    // Git uses the index timestamp to detect potentially stale cached file stats.
    copy.set_modified(modified)?;
    Ok(())
}

fn selected_paths(
    root: &Path,
    output_dir: &Path,
    candidates: &[u8],
    tracked: &[u8],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let artifacts = artifacts::Artifacts::new(output_dir);
    let tracked: HashSet<_> = tracked.split(|byte| *byte == 0).collect();
    let mut paths = Vec::new();
    for name in candidates
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        let path = root.join(path_from_bytes(name.to_vec())?);
        if tracked.contains(name) || !artifacts.contains(&path) {
            paths.extend_from_slice(path.as_os_str().as_encoded_bytes());
            paths.push(0);
        }
    }
    Ok(paths)
}

fn add_intent(git: &OsStr, cwd: &Path, index: &Path, paths: &[u8]) -> Result<(), Box<dyn Error>> {
    let mut child = Command::new(git)
        .current_dir(cwd)
        .env("GIT_INDEX_FILE", index)
        .env("GIT_LITERAL_PATHSPECS", "1")
        .args([
            "-c",
            "core.splitIndex=false",
            "-c",
            "index.sparse=false",
            "add",
            "--intent-to-add",
            "--sparse",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("failed to start git add: {error}"))?;
    let write = child.stdin.take().expect("piped stdin").write_all(paths);
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("git add --intent-to-add failed with {status}").into());
    }
    write?;
    Ok(())
}

fn checked(command: &mut Command) -> Result<Output, Box<dyn Error>> {
    let output = command
        .output()
        .map_err(|error| format!("failed to start git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Git operation {:?} failed with {}: {}",
            command.get_args().collect::<Vec<_>>(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    // Successful Git commands can still produce useful diagnostics.
    io::stderr().write_all(&output.stderr)?;
    Ok(output)
}

fn trim_newline(mut bytes: Vec<u8>) -> Vec<u8> {
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    bytes
}

fn path_from_bytes(bytes: Vec<u8>) -> Result<PathBuf, Box<dyn Error>> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(OsString::from_vec(bytes).into())
    }
    #[cfg(not(unix))]
    {
        Ok(OsString::from(String::from_utf8(bytes)?).into())
    }
}
