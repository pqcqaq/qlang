use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const TEMP_FILE_ATTEMPTS: u32 = 16;

pub fn write_file_atomically(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let temp_path = create_temp_file(path, contents.as_ref())?;
    replace_file_atomically(path, &temp_path)
}

pub(crate) fn replace_file_atomically(path: &Path, temp_path: &Path) -> io::Result<()> {
    if let Err(error) = OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp_path)
        .and_then(|file| file.sync_all())
    {
        let _ = fs::remove_file(temp_path);
        return Err(error);
    }
    match replace_file(path, &temp_path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(temp_path);
            Err(error)
        }
    }
}

fn create_temp_file(path: &Path, contents: &[u8]) -> io::Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("output");

    for attempt in 0..TEMP_FILE_ATTEMPTS {
        let temp_path = parent.join(format!(
            ".{file_name}.{}.{}.ql.tmp",
            std::process::id(),
            unique_timestamp_nanos() + u128::from(attempt)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(contents) {
                    let _ = fs::remove_file(&temp_path);
                    return Err(error);
                }
                if let Err(error) = file.sync_all() {
                    let _ = fs::remove_file(&temp_path);
                    return Err(error);
                }
                return Ok(temp_path);
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        format!(
            "failed to reserve a unique temporary output next to `{}`",
            path.display()
        ),
    ))
}

fn unique_timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos()
}

#[cfg(not(windows))]
fn replace_file(path: &Path, temp_path: &Path) -> io::Result<()> {
    fs::rename(temp_path, path)
}

#[cfg(windows)]
fn replace_file(path: &Path, temp_path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    let old = temp_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let new = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    let result = unsafe {
        move_file_ex_w(
            old.as_ptr(),
            new.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    #[link_name = "MoveFileExW"]
    fn move_file_ex_w(existing_file_name: *const u16, new_file_name: *const u16, flags: u32)
    -> i32;
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{replace_file_atomically, write_file_atomically};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).expect("create temporary test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn atomic_write_replaces_existing_file_without_leaking_temp_file() {
        let dir = TestDir::new("ql-driver-atomic-write");
        let path = dir.path().join("artifact.txt");
        fs::write(&path, "old").expect("write initial file");

        write_file_atomically(&path, "new").expect("replace file atomically");

        assert_eq!(
            fs::read_to_string(&path).expect("read replaced file"),
            "new"
        );
        let leaked = fs::read_dir(dir.path())
            .expect("read temp dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .find(|name| name.ends_with(".ql.tmp"));
        assert_eq!(leaked, None, "atomic write temp file should be removed");
    }

    #[test]
    fn atomic_replace_promotes_existing_temp_file_without_leaking_it() {
        let dir = TestDir::new("ql-driver-atomic-replace");
        let path = dir.path().join("artifact.bin");
        let temp_path = dir.path().join("artifact.tmp");
        fs::write(&path, "old").expect("write initial file");
        fs::write(&temp_path, "new").expect("write temp file");

        replace_file_atomically(&path, &temp_path).expect("replace file atomically");

        assert_eq!(
            fs::read_to_string(&path).expect("read replaced file"),
            "new"
        );
        assert!(
            !temp_path.exists(),
            "promoted temp artifact should be removed"
        );
    }
}
