use std::{error::Error, ffi::OsStr, path::Path, process::Command};

pub(super) fn require_supported(cwd: &Path, git: &OsStr) -> Result<(), Box<dyn Error>> {
    let output = super::checked(Command::new(git).current_dir(cwd).arg("--version"))?;
    let version = String::from_utf8_lossy(&output.stdout);
    if !supported(version.trim()) {
        return Err(format!(
            "Including untracked files requires Git 2.34 or newer (found {:?}). Upgrade Git or use 'gd all --tracked-only'.",
            version.trim()
        ).into());
    }
    Ok(())
}

fn supported(output: &str) -> bool {
    let Some(release) = output
        .strip_prefix("git version ")
        .and_then(|text| text.split_whitespace().next())
    else {
        return false;
    };
    let mut parts = release.split('.');
    let major = parts.next().and_then(|part| part.parse::<u64>().ok());
    let minor = parts.next().and_then(|part| part.parse::<u64>().ok());
    major.zip(minor).is_some_and(|version| version >= (2, 34))
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_detection_accepts_platform_suffixes_and_rejects_unsupported_output() {
        for (version, supported) in [
            ("git version 2.34.0", true),
            ("git version 2.34.1.windows.1", true),
            ("git version 2.39.5 (Apple Git-154)", true),
            ("git version 2.34.0-GIT", true),
            ("git version 3.0.0", true),
            ("git version 2.33.9.windows.1", false),
            ("git version 1.99.9", false),
            ("git version unknown", false),
            ("git version 2", false),
            ("git version 2.bad.0", false),
            ("not a Git version", false),
        ] {
            assert_eq!(super::supported(version), supported, "{version}");
        }
    }
}
