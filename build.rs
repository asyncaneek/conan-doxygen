use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

const COPY_DIR: &str = "./template";

/// A helper function for recursively copying a directory.
fn copy_dir<P, Q>(from: P, to: Q) -> io::Result<()>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let to = to.as_ref().to_path_buf();

    for path in fs::read_dir(from)? {
        let path = path?.path();
        let to = to.clone().join(
            path.file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Invalid file name"))?,
        );

        if path.is_file() {
            fs::copy(&path, to)?;
        } else if path.is_dir() {
            if !to.exists() {
                fs::create_dir(&to)?;
            }

            copy_dir(&path, to)?;
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    // Request the output directory
    let out = env::var("PROFILE").expect("PROFILE not found");
    let out = PathBuf::from(format!("target/{}/{}", out, COPY_DIR));

    // If it is already in the output directory, delete it and start over
    if out.exists() {
        fs::remove_dir_all(&out)?;
    }

    // Create the out directory
    fs::create_dir(&out)?;

    // Copy the directory
    copy_dir(COPY_DIR, &out)?;

    Ok(())
}
