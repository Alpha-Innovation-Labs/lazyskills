use std::io;
use std::path::PathBuf;

pub fn default_user_data_dir() -> io::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return dirs::data_local_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Unable to resolve LOCALAPPDATA directory",
            )
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg_data_home) = std::env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(xdg_data_home));
        }

        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home).join(".local").join("share"));
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Unable to resolve HOME/XDG_DATA_HOME",
        ))
    }
}
