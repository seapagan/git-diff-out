use std::{error::Error, path::PathBuf};

use clap::Parser;

mod man;

#[derive(Parser)]
struct Arguments {
    #[arg(value_name = "OUTPUT")]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = Arguments::parse();
    man::generate(&arguments.output)
}

#[cfg(all(test, unix))]
mod tests {
    use std::{ffi::OsString, fs, os::unix::ffi::OsStringExt};

    use clap::Parser;

    use super::{Arguments, man};

    #[test]
    fn non_utf8_output_path_is_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(OsString::from_vec(b"gd-\x80.1".to_vec()));
        let arguments = Arguments::try_parse_from([
            OsString::from("generate-man"),
            path.clone().into_os_string(),
        ])
        .unwrap();

        man::generate(&arguments.output).unwrap();

        assert_eq!(fs::read(path).unwrap(), man::render_manual().unwrap());
    }
}
