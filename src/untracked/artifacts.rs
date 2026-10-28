use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) struct Artifacts {
    destination: Option<PathBuf>,
    #[cfg(unix)]
    stdout: Option<fs::Metadata>,
    #[cfg(windows)]
    stdout: Option<PathBuf>,
}

impl Artifacts {
    pub(super) fn new(output_dir: &Path) -> Self {
        Self {
            destination: destination_path(&output_dir.join("uncommitted.diff")),
            stdout: stdout_file(),
        }
    }

    pub(super) fn contains(&self, path: &Path) -> bool {
        if self
            .destination
            .as_ref()
            .is_some_and(|destination| destination_path(path).as_ref() == Some(destination))
        {
            return true;
        }
        self.is_stdout(path)
    }

    #[cfg(unix)]
    fn is_stdout(&self, path: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        self.stdout.as_ref().is_some_and(|stdout| {
            fs::symlink_metadata(path).is_ok_and(|file| {
                file.is_file() && file.dev() == stdout.dev() && file.ino() == stdout.ino()
            })
        })
    }

    #[cfg(windows)]
    fn is_stdout(&self, path: &Path) -> bool {
        self.stdout.as_ref().is_some_and(|stdout| {
            fs::symlink_metadata(path).is_ok_and(|file| file.is_file())
                && fs::canonicalize(path).ok().as_ref() == Some(stdout)
        })
    }
}

// Resolve directory aliases without following the destination symlink itself.
fn destination_path(path: &Path) -> Option<PathBuf> {
    Some(
        fs::canonicalize(path.parent()?)
            .ok()?
            .join(path.file_name()?),
    )
}

#[cfg(unix)]
fn stdout_file() -> Option<fs::Metadata> {
    use std::{io, os::fd::AsFd};
    let file = fs::File::from(io::stdout().as_fd().try_clone_to_owned().ok()?);
    file.metadata().ok().filter(fs::Metadata::is_file)
}

#[cfg(windows)]
fn stdout_file() -> Option<PathBuf> {
    use std::{
        ffi::c_void,
        io,
        os::windows::{ffi::OsStringExt, io::AsRawHandle},
    };
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFinalPathNameByHandleW(
            handle: *mut c_void,
            path: *mut u16,
            length: u32,
            flags: u32,
        ) -> u32;
    }
    let mut buffer = vec![0_u16; 32768];
    // The handle is borrowed from stdout; the buffer is writable for its stated length.
    let length = unsafe {
        GetFinalPathNameByHandleW(
            io::stdout().as_raw_handle(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            0,
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        return None;
    }
    Some(std::ffi::OsString::from_wide(&buffer[..length]).into())
}
