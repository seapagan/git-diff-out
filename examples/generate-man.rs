use std::{env, error::Error, io, path::Path};

mod man;

fn main() -> Result<(), Box<dyn Error>> {
    let output = env::args_os()
        .nth(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "usage: generate-man OUTPUT"))?;
    if env::args_os().nth(2).is_some() {
        return Err(
            io::Error::new(io::ErrorKind::InvalidInput, "usage: generate-man OUTPUT").into(),
        );
    }
    man::generate(Path::new(&output))
}
