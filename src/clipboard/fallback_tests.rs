use super::{
    Backend::{self, Osc52, Pbcopy, Windows, WlCopy, Xclip, Xsel},
    ClipboardError, copy_with_fallback,
};

const LOCAL_BACKENDS: [Backend; 5] = [WlCopy, Xclip, Xsel, Pbcopy, Windows];

#[test]
fn runtime_provider_failure_does_not_fallback_without_opt_in() {
    for backend in LOCAL_BACKENDS {
        let mut attempts = Vec::new();
        let error = copy_with_fallback(backend, b"diff", false, &mut |attempt, _payload| {
            attempts.push(attempt);
            Err(ClipboardError("runtime failure".into()))
        })
        .unwrap_err()
        .to_string();

        assert_eq!(attempts, [backend]);
        assert_eq!(error, "runtime failure");
    }
}

#[test]
fn runtime_provider_failure_falls_back_to_osc52_when_enabled() {
    for backend in LOCAL_BACKENDS {
        let mut attempts = Vec::new();
        copy_with_fallback(backend, b"diff", true, &mut |attempt, _payload| {
            attempts.push(attempt);
            if attempt == Osc52 {
                Ok(())
            } else {
                Err(ClipboardError("runtime failure".into()))
            }
        })
        .unwrap();

        assert_eq!(attempts, [backend, Osc52]);
    }
}

#[test]
fn runtime_osc52_failure_is_not_retried() {
    let mut attempts = Vec::new();
    let error = copy_with_fallback(Osc52, b"diff", true, &mut |attempt, _payload| {
        attempts.push(attempt);
        Err(ClipboardError("runtime failure".into()))
    })
    .unwrap_err()
    .to_string();

    assert_eq!(attempts, [Osc52]);
    assert_eq!(error, "runtime failure");
}
