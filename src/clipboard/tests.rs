#[cfg(unix)]
use super::run_provider;
use super::{
    Backend, Platform, Selection, clipboard_text, encode_osc52, provider_spec, select_backend,
    write_osc52, write_osc52_path,
};

fn local_linux() -> Selection {
    Selection {
        platform: Platform::Linux,
        ssh: false,
        wayland: false,
        x11: false,
        wl_copy: false,
        xclip: false,
        xsel: false,
        pbcopy: false,
        osc52_fallback: false,
    }
}

#[test]
fn ssh_always_selects_osc52() {
    for platform in [Platform::Linux, Platform::Macos, Platform::Windows] {
        assert_eq!(
            select_backend(&Selection {
                platform,
                ssh: true,
                ..local_linux()
            })
            .unwrap(),
            Backend::Osc52
        );
    }
}

#[test]
fn linux_provider_selection_respects_the_display_session() {
    let all = Selection {
        wayland: true,
        x11: true,
        wl_copy: true,
        xclip: true,
        xsel: true,
        ..local_linux()
    };
    assert_eq!(select_backend(&all).unwrap(), Backend::WlCopy);
    assert_eq!(
        select_backend(&Selection {
            wl_copy: false,
            ..all
        })
        .unwrap(),
        Backend::Xclip
    );
    assert_eq!(
        select_backend(&Selection {
            wayland: false,
            wl_copy: false,
            xclip: false,
            ..all
        })
        .unwrap(),
        Backend::Xsel
    );
}

#[test]
fn linux_does_not_invoke_a_provider_for_an_incompatible_session() {
    let error = select_backend(&Selection {
        wl_copy: true,
        xclip: true,
        xsel: true,
        ..local_linux()
    })
    .unwrap_err()
    .to_string();

    assert!(error.contains("wl-clipboard"), "{error}");
    assert!(error.contains("xclip"), "{error}");
    assert!(error.contains("xsel"), "{error}");
}

#[test]
fn local_osc52_fallback_requires_opt_in() {
    assert!(select_backend(&local_linux()).is_err());
    assert_eq!(
        select_backend(&Selection {
            osc52_fallback: true,
            ..local_linux()
        })
        .unwrap(),
        Backend::Osc52
    );
}

#[test]
fn macos_and_windows_use_native_backends() {
    assert_eq!(
        select_backend(&Selection {
            platform: Platform::Macos,
            pbcopy: true,
            ..local_linux()
        })
        .unwrap(),
        Backend::Pbcopy
    );
    assert_eq!(
        select_backend(&Selection {
            platform: Platform::Windows,
            ..local_linux()
        })
        .unwrap(),
        Backend::Windows
    );
}

#[test]
fn osc52_encodes_arbitrary_utf8_for_the_standard_clipboard() {
    assert_eq!(
        encode_osc52("diff café 😀\n".as_bytes()).unwrap(),
        b"\x1b]52;c;ZGlmZiBjYWbDqSDwn5iACg==\x1b\\"
    );
}

#[test]
fn osc52_rejects_oversize_payloads_without_truncating() {
    let mut payload = Vec::new();
    payload.resize(74_991, b'x');
    let sequence = encode_osc52(&payload).unwrap();
    assert_eq!(sequence.len(), 99_997);

    payload.push(b'x');
    let error = encode_osc52(&payload).unwrap_err().to_string();
    assert!(error.contains("74992 bytes"), "{error}");
    assert!(error.contains("maximum is 74991 bytes"), "{error}");
}

#[test]
fn osc52_writes_only_to_the_supplied_terminal() {
    let mut terminal = Vec::new();
    write_osc52(b"diff\n", &mut terminal).unwrap();

    assert_eq!(terminal, b"\x1b]52;c;ZGlmZgo=\x1b\\");
}

#[test]
fn osc52_reports_a_missing_controlling_terminal() {
    let error = write_osc52_path(
        std::path::Path::new("/definitely/missing/gd-controlling-terminal"),
        b"diff",
    )
    .unwrap_err()
    .to_string();

    assert!(error.contains("controlling terminal"), "{error}");
}

#[test]
fn windows_text_conversion_preserves_unicode() {
    assert_eq!(clipboard_text("café 😀".as_bytes()).unwrap(), "café 😀");
}

#[test]
fn windows_text_conversion_rejects_invalid_utf8_and_embedded_nul() {
    assert!(clipboard_text(&[0xff]).is_err());
    assert!(clipboard_text(b"before\0after").is_err());
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
