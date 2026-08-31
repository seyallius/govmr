use crate::errors::GovmError;
use std::{
    env,
    fs::{self, File},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

pub struct ShimManager {
    shim_dir: PathBuf,
}
impl ShimManager {
    pub fn new() -> Result<Self, GovmError> {
        let home = dirs::home_dir().ok_or(GovmError::HomeNotFound)?;
        let shim_dir = home.join(".govm").join("shim");
        fs::create_dir_all(&shim_dir)?;
        Ok(Self { shim_dir })
    }

    pub fn get_shim_dir(&self) -> &Path {
        &self.shim_dir
    }

    pub fn is_in_path(&self) -> bool {
        if let Some(paths) = env::var_os("PATH") {
            for path in env::split_paths(&paths) {
                if path == self.shim_dir {
                    return true;
                }
            }
        }
        false
    }

    pub fn setup_shims_for_version(&self, bin_dir: &Path) -> Result<(), GovmError> {
        for entry in fs::read_dir(bin_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let bin_name = match path.file_name() {
                    Some(name) => name.to_string_lossy().to_string(),
                    None => continue,
                };
                self.create_shim(&bin_name, &path)?;
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    fn create_shim(&self, bin_name: &str, target_path: &Path) -> Result<(), GovmError> {
        let shim_path = self.shim_dir.join(bin_name);
        let content = format!(
            "#!/usr/bin/env bash\n\"{}\" \"$@\"\n",
            target_path.display()
        );
        let mut file = File::create(&shim_path)?;
        file.write_all(content.as_bytes())?;
        fs::set_permissions(&shim_path, fs::Permissions::from_mode(0o755))?;
        Ok(())
    }

    #[cfg(windows)]
    fn create_shim(&self, bin_name: &str, target_path: &Path) -> Result<(), GovmError> {
        let shim_path = self.shim_dir.join(format!("{}.bat", bin_name));
        let content = format!("@echo off\r\n\"{}\" %*\r\n", target_path.display());
        let mut file = File::create(&shim_path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }
}
