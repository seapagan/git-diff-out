use std::{fs, path::PathBuf};

use tempfile::TempDir;

pub fn executable_script(name: &str) -> (TempDir, PathBuf) {
    let root = tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/target")).unwrap();
    let path = root.path().join(name);
    fs::hard_link(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fake-git.sh"),
        &path,
    )
    .unwrap();
    (root, path)
}
