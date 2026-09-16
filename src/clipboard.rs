use std::{
    env,
    error::Error,
    ffi::OsStr,
    fmt,
    fs::OpenOptions,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

// TODO: Once gd's API and cross-platform behavior are proven stable, consider
// extracting this CLI-oriented clipboard router into a standalone Rust crate.
const OSC52_MAX_BYTES: usize = 74_991;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Backend {
    Osc52,
    WlCopy,
    Xclip,
    Xsel,
    Pbcopy,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Platform {
    Linux,
    Macos,
    Windows,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Selection {
    pub platform: Platform,
    pub ssh: bool,
    pub wayland: bool,
    pub x11: bool,
    pub wl_copy: bool,
    pub xclip: bool,
    pub xsel: bool,
    pub pbcopy: bool,
    pub osc52_fallback: bool,
}

#[derive(Debug)]
pub(crate) struct ClipboardError(String);

impl fmt::Display for ClipboardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ClipboardError {}

pub(crate) fn select_backend(selection: &Selection) -> Result<Backend, ClipboardError> {
    if selection.ssh {
        return Ok(Backend::Osc52);
    }
    match selection.platform {
        Platform::Linux => {
            if selection.wayland && selection.wl_copy {
                return Ok(Backend::WlCopy);
            }
            if selection.x11 && selection.xclip {
                return Ok(Backend::Xclip);
            }
            if selection.x11 && selection.xsel {
                return Ok(Backend::Xsel);
            }
        }
        Platform::Macos if selection.pbcopy => return Ok(Backend::Pbcopy),
        Platform::Windows => return Ok(Backend::Windows),
        Platform::Macos => {}
    }
    if selection.osc52_fallback {
        return Ok(Backend::Osc52);
    }
    Err(ClipboardError(match selection.platform {
        Platform::Linux => "no supported local clipboard provider is available; install wl-clipboard for Wayland or xclip/xsel for X11".into(),
        Platform::Macos => "the macOS clipboard provider 'pbcopy' is not available".into(),
        Platform::Windows => unreachable!("Windows always has a native backend"),
    }))
}

pub(crate) fn encode_osc52(payload: &[u8]) -> Result<Vec<u8>, ClipboardError> {
    if payload.len() > OSC52_MAX_BYTES {
        return Err(ClipboardError(format!(
            "OSC 52 payload is too large ({} bytes; maximum is {OSC52_MAX_BYTES} bytes); clipboard content was not sent",
            payload.len()
        )));
    }

    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = Vec::with_capacity(payload.len().div_ceil(3) * 4 + 9);
    encoded.extend_from_slice(b"\x1b]52;c;");
    for chunk in payload.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        encoded.push(TABLE[((value >> 18) & 63) as usize]);
        encoded.push(TABLE[((value >> 12) & 63) as usize]);
        encoded.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize]
        } else {
            b'='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize]
        } else {
            b'='
        });
    }
    encoded.extend_from_slice(b"\x1b\\");
    Ok(encoded)
}

fn write_osc52(payload: &[u8], terminal: &mut dyn Write) -> Result<(), ClipboardError> {
    let sequence = encode_osc52(payload)?;
    terminal
        .write_all(&sequence)
        .and_then(|()| terminal.flush())
        .map_err(|error| {
            ClipboardError(format!(
                "cannot write OSC 52 to the controlling terminal: {error}"
            ))
        })
}

fn write_osc52_path(path: &Path, payload: &[u8]) -> Result<(), ClipboardError> {
    let mut terminal = OpenOptions::new().write(true).open(path).map_err(|error| {
        ClipboardError(format!(
            "cannot open a controlling terminal for OSC 52 at '{}': {error}",
            path.display()
        ))
    })?;
    write_osc52(payload, &mut terminal)
}

#[cfg(any(windows, test))]
fn utf16_nul(payload: &[u8]) -> Result<Vec<u16>, ClipboardError> {
    let text = std::str::from_utf8(payload).map_err(|error| {
        ClipboardError(format!("clipboard content is not valid UTF-8: {error}"))
    })?;
    if text.contains('\0') {
        return Err(ClipboardError(
            "clipboard content contains an embedded NUL character".into(),
        ));
    }
    Ok(text.encode_utf16().chain([0]).collect())
}

fn provider_spec(backend: Backend) -> (&'static str, &'static [&'static str]) {
    match backend {
        Backend::WlCopy => ("wl-copy", &[]),
        Backend::Xclip => ("xclip", &["-selection", "clipboard", "-in"]),
        Backend::Xsel => ("xsel", &["--clipboard", "--input"]),
        Backend::Pbcopy => ("pbcopy", &[]),
        Backend::Osc52 | Backend::Windows => unreachable!("not a command provider"),
    }
}

fn run_provider(
    name: &str,
    program: &str,
    args: &[&str],
    payload: &[u8],
) -> Result<(), ClipboardError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| {
            ClipboardError(format!("cannot start clipboard provider '{name}': {error}"))
        })?;
    let write_result = child
        .stdin
        .take()
        .expect("piped provider stdin is available")
        .write_all(payload);
    let status = child.wait().map_err(|error| {
        ClipboardError(format!(
            "cannot wait for clipboard provider '{name}': {error}"
        ))
    })?;
    write_result.map_err(|error| {
        ClipboardError(format!(
            "cannot write to clipboard provider '{name}': {error}"
        ))
    })?;
    if !status.success() {
        return Err(ClipboardError(format!(
            "clipboard provider '{name}' failed with {status}"
        )));
    }
    Ok(())
}

pub(crate) fn copy(payload: &[u8], osc52_fallback: bool) -> Result<(), ClipboardError> {
    let selection = system_selection(osc52_fallback);
    let backend = select_backend(&selection)?;
    let result = copy_with_backend(backend, payload);
    if result.is_err() && osc52_fallback && backend == Backend::Windows {
        return copy_with_backend(Backend::Osc52, payload);
    }
    result
}

fn copy_with_backend(backend: Backend, payload: &[u8]) -> Result<(), ClipboardError> {
    match backend {
        Backend::Osc52 => write_osc52_path(controlling_terminal(), payload),
        Backend::WlCopy | Backend::Xclip | Backend::Xsel | Backend::Pbcopy => {
            let (program, args) = provider_spec(backend);
            run_provider(program, program, args, payload)
        }
        Backend::Windows => copy_windows(payload),
    }
}

fn system_selection(osc52_fallback: bool) -> Selection {
    Selection {
        platform: current_platform(),
        ssh: ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
            .iter()
            .any(|name| env_nonempty(name)),
        wayland: env_nonempty("WAYLAND_DISPLAY")
            || env::var("XDG_SESSION_TYPE")
                .is_ok_and(|value| value.eq_ignore_ascii_case("wayland")),
        x11: env_nonempty("DISPLAY"),
        wl_copy: program_exists("wl-copy"),
        xclip: program_exists("xclip"),
        xsel: program_exists("xsel"),
        pbcopy: program_exists("pbcopy"),
        osc52_fallback,
    }
}

fn env_nonempty(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn program_exists(program: impl AsRef<OsStr>) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|directory| directory.join(program.as_ref()).is_file())
}

fn current_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

#[cfg(windows)]
fn controlling_terminal() -> &'static Path {
    Path::new("CONOUT$")
}

#[cfg(not(windows))]
fn controlling_terminal() -> &'static Path {
    Path::new("/dev/tty")
}

#[cfg(not(windows))]
fn copy_windows(_payload: &[u8]) -> Result<(), ClipboardError> {
    unreachable!("Windows backend is only selected on Windows")
}

#[cfg(windows)]
fn copy_windows(payload: &[u8]) -> Result<(), ClipboardError> {
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::GlobalFree,
        System::{
            Console::GetConsoleWindow,
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
            Ole::CF_UNICODETEXT,
        },
    };

    let wide = utf16_nul(payload)?;
    // SAFETY: The console window handle is checked before it owns the clipboard. The
    // movable allocation is locked while its complete NUL-terminated UTF-16 payload is
    // copied. It is freed on every pre-transfer failure and deliberately retained after
    // SetClipboardData succeeds.
    unsafe {
        let owner = GetConsoleWindow();
        if owner.is_null() {
            return Err(ClipboardError(
                "cannot open the Windows clipboard because gd has no console window".into(),
            ));
        }
        if OpenClipboard(owner) == 0 {
            return Err(last_windows_error("cannot open the Windows clipboard"));
        }
        let result = (|| {
            if EmptyClipboard() == 0 {
                return Err(last_windows_error("cannot empty the Windows clipboard"));
            }
            let memory = GlobalAlloc(GMEM_MOVEABLE, wide.len() * size_of::<u16>());
            if memory.is_null() {
                return Err(last_windows_error(
                    "cannot allocate Windows clipboard memory",
                ));
            }
            let locked = GlobalLock(memory).cast::<u16>();
            if locked.is_null() {
                let error = last_windows_error("cannot lock Windows clipboard memory");
                GlobalFree(memory);
                return Err(error);
            }
            ptr::copy_nonoverlapping(wide.as_ptr(), locked, wide.len());
            GlobalUnlock(memory);
            if SetClipboardData(u32::from(CF_UNICODETEXT), memory).is_null() {
                let error = last_windows_error("cannot set Unicode Windows clipboard data");
                GlobalFree(memory);
                return Err(error);
            }
            Ok(())
        })();
        let close_result = if CloseClipboard() == 0 {
            Err(last_windows_error("cannot close the Windows clipboard"))
        } else {
            Ok(())
        };
        result.and(close_result)
    }
}

#[cfg(windows)]
fn last_windows_error(context: &str) -> ClipboardError {
    ClipboardError(format!("{context}: {}", std::io::Error::last_os_error()))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::run_provider;
    use super::{
        Backend, Platform, Selection, encode_osc52, provider_spec, select_backend, utf16_nul,
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
        let sequence = encode_osc52(&vec![b'x'; 74_991]).unwrap();
        assert_eq!(sequence.len(), 99_997);

        let error = encode_osc52(&vec![b'x'; 74_992]).unwrap_err().to_string();
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
    fn windows_text_conversion_preserves_unicode_and_adds_one_nul() {
        assert_eq!(
            utf16_nul("café 😀".as_bytes()).unwrap(),
            [0x63, 0x61, 0x66, 0xe9, 0x20, 0xd83d, 0xde00, 0]
        );
    }

    #[test]
    fn windows_text_conversion_rejects_invalid_utf8_and_embedded_nul() {
        assert!(utf16_nul(&[0xff]).is_err());
        assert!(utf16_nul(b"before\0after").is_err());
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
        let error = run_provider("fake", "/bin/sh", &["-c", "exit 7"], b"diff")
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
}
