#[cfg(unix)]
use super::run_provider;
use super::{Backend, program_exists_in, provider_spec};

#[test]
fn provider_discovery_handles_missing_and_populated_paths() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("wl-copy"), []).unwrap();
    let path = std::env::join_paths([temp.path()]).unwrap();

    assert!(!program_exists_in(None, std::ffi::OsStr::new("wl-copy")));
    assert!(program_exists_in(
        Some(path.as_os_str()),
        std::ffi::OsStr::new("wl-copy")
    ));
    assert!(!program_exists_in(
        Some(path.as_os_str()),
        std::ffi::OsStr::new("xclip")
    ));
}

#[test]
fn provider_commands_select_the_system_clipboard() {
    assert_eq!(provider_spec(Backend::WlCopy), ("wl-copy", &[][..]));
    assert_eq!(
        provider_spec(Backend::Xclip),
        ("xclip", &["-selection", "clipboard", "-in"][..])
    );
    assert_eq!(
        provider_spec(Backend::Xsel),
        ("xsel", &["--clipboard", "--input"][..])
    );
    assert_eq!(provider_spec(Backend::Pbcopy), ("pbcopy", &[][..]));
}

#[test]
#[cfg(unix)]
fn providers_receive_the_exact_payload() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("clipboard.bin");
    let destination = destination.to_str().unwrap();
    let payload = "diff café 😀\n".as_bytes();

    run_provider(
        "fake",
        "/bin/sh",
        &["-c", "cat > \"$1\"", "sh", destination],
        payload,
    )
    .unwrap();

    assert_eq!(std::fs::read(destination).unwrap(), payload);
}

#[test]
#[cfg(unix)]
fn provider_startup_failures_name_the_backend() {
    let error = run_provider(
        "missing-provider",
        "/definitely/missing/gd-clipboard-provider",
        &[],
        b"diff",
    )
    .unwrap_err()
    .to_string();

    assert!(
        error.starts_with("cannot start clipboard provider 'missing-provider':"),
        "{error}"
    );
}

#[test]
#[cfg(unix)]
fn provider_stdin_failures_name_the_backend() {
    let payload = vec![b'x'; 1024 * 1024];
    let error = run_provider(
        "closed-stdin",
        "/bin/sh",
        &["-c", "exec 0<&-; exit 0"],
        &payload,
    )
    .unwrap_err()
    .to_string();

    assert!(
        error.starts_with("cannot write to clipboard provider 'closed-stdin':"),
        "{error}"
    );
}

#[test]
#[cfg(unix)]
fn provider_failures_name_the_backend_and_status() {
    let error = run_provider(
        "fake",
        "/bin/sh",
        &["-c", "cat >/dev/null; exit 7"],
        b"diff",
    )
    .unwrap_err()
    .to_string();

    assert!(
        error.contains("clipboard provider 'fake' failed"),
        "{error}"
    );
    assert!(error.contains('7'), "{error}");
}
