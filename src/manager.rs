use crate::{
    errors::GovmError,
    models::{GoRelease, GoVersion},
    shim::ShimManager,
};
use futures::StreamExt;
use std::{
    env::consts::{ARCH, OS},
    fs::{self, File},
    path::PathBuf,
};
use tokio::io::AsyncWriteExt;

pub struct GoManager {
    base_dir: PathBuf,
    versions_dir: PathBuf,
    downloads_dir: PathBuf,
    shim_mgr: ShimManager,
    client: reqwest::Client,
}
impl GoManager {
    pub fn new() -> Result<Self, GovmError> {
        let home = dirs::home_dir().ok_or(GovmError::HomeNotFound)?;
        let base_dir = home.join(".govm");
        let versions_dir = base_dir.join("versions");
        let downloads_dir = base_dir.join("downloads");

        fs::create_dir_all(&versions_dir)?;
        fs::create_dir_all(&downloads_dir)?;

        Ok(Self {
            base_dir,
            versions_dir,
            downloads_dir,
            shim_mgr: ShimManager::new()?,
            client: reqwest::Client::builder().build()?,
        })
    }

    pub fn get_shim_manager(&self) -> &ShimManager {
        &self.shim_mgr
    }

    pub fn get_active_version(&self) -> Option<String> {
        let active_file = self.base_dir.join("active_version");
        fs::read_to_string(active_file)
            .ok()
            .map(|v| v.trim().to_string())
    }

    pub async fn fetch_versions(&self) -> Result<Vec<GoVersion>, GovmError> {
        let url = "https://go.dev/dl/?mode=json&include=all";
        let releases: Vec<GoRelease> = self.client.get(url).send().await?.json().await?;

        let go_os = match OS {
            "macos" => "darwin",
            other => other,
        };
        let go_arch = match ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => other,
        };

        let active_version = self.get_active_version();
        let mut versions = Vec::new();

        for release in releases {
            let ver_clean = release.version.trim_start_matches("go").to_string();
            if let Some(file) = release
                .files
                .into_iter()
                .find(|f| f.os == go_os && f.arch == go_arch)
            {
                let install_dir = self.versions_dir.join(format!("go{}", ver_clean));
                let installed = install_dir.join("bin").exists();
                let active = active_version.as_deref() == Some(&ver_clean);

                versions.push(GoVersion {
                    raw_version: ver_clean.clone(),
                    display_name: format!("go{}", ver_clean),
                    filename: file.filename.clone(),
                    url: format!("https://go.dev/dl/{}", file.filename),
                    installed,
                    active,
                    path: if installed { Some(install_dir) } else { None },
                    stable: release.stable,
                });
            }
        }

        versions.sort_by(|a, b| Self::compare_versions(&b.raw_version, &a.raw_version));
        Ok(versions)
    }

    pub async fn download_and_install<F>(
        &self,
        version: &GoVersion,
        progress: F,
    ) -> Result<PathBuf, GovmError>
    where
        F: Fn(f64) + Send + 'static,
    {
        let download_path = self.downloads_dir.join(&version.filename);
        let target_dir = self.versions_dir.join(format!("go{}", version.raw_version));

        let res = self.client.get(&version.url).send().await?;
        let total_size = res.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut stream = res.bytes_stream();

        let mut file = tokio::fs::File::create(&download_path).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if total_size > 0 {
                progress(downloaded as f64 / total_size as f64);
            }
        }
        file.flush().await?;

        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)?;
        }
        fs::create_dir_all(&target_dir)?;

        let dl_path_clone = download_path.clone();
        let target_dir_clone = target_dir.clone();
        let is_tar = version.filename.ends_with(".tar.gz");

        tokio::task::spawn_blocking(move || -> Result<(), GovmError> {
            if is_tar {
                let tar_gz = File::open(&dl_path_clone)?;
                let tar = flate2::read::GzDecoder::new(tar_gz);
                let mut archive = tar::Archive::new(tar);

                // Extract stripping the root "go/" wrapper folder
                for entry in archive.entries()? {
                    let mut entry = entry?;
                    let path = entry.path()?;
                    let stripped: PathBuf = path.components().skip(1).collect();
                    if !stripped.as_os_str().is_empty() {
                        let out_path = target_dir_clone.join(stripped);
                        if entry.header().entry_type().is_dir() {
                            fs::create_dir_all(&out_path)?;
                        } else {
                            if let Some(p) = out_path.parent() {
                                fs::create_dir_all(p)?;
                            }
                            entry.unpack(&out_path)?;
                        }
                    }
                }
            } else {
                let zip_file = File::open(&dl_path_clone)?;
                let mut archive = zip::ZipArchive::new(zip_file)
                    .map_err(|e| GovmError::Extraction(e.to_string()))?;
                for i in 0..archive.len() {
                    let mut file = archive
                        .by_index(i)
                        .map_err(|e| GovmError::Extraction(e.to_string()))?;
                    let outpath = match file.enclosed_name() {
                        Some(path) => {
                            let stripped: PathBuf = path.components().skip(1).collect();
                            target_dir_clone.join(stripped)
                        }
                        None => continue,
                    };
                    if (*file.name()).ends_with('/') {
                        fs::create_dir_all(&outpath)?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            fs::create_dir_all(p)?;
                        }
                        let mut outfile = File::create(&outpath)?;
                        std::io::copy(&mut file, &mut outfile)?;
                    }
                }
            }
            let _ = fs::remove_file(dl_path_clone);
            Ok(())
        })
        .await
        .map_err(|e| GovmError::Extraction(e.to_string()))??;

        Ok(target_dir)
    }

    pub fn switch_version(&self, version: &GoVersion) -> Result<bool, GovmError> {
        let version_path = version
            .path
            .as_ref()
            .ok_or_else(|| GovmError::NotInstalled(version.raw_version.clone()))?;
        let bin_dir = version_path.join("bin");

        self.shim_mgr.setup_shims_for_version(&bin_dir)?;

        let active_file = self.base_dir.join("active_version");
        fs::write(active_file, &version.raw_version)?;

        Ok(self.shim_mgr.is_in_path())
    }

    pub fn delete_version(&self, version: &GoVersion) -> Result<(), GovmError> {
        if !version.installed {
            return Err(GovmError::NotInstalled(version.raw_version.clone()));
        }
        if version.active {
            return Err(GovmError::CannotDeleteActive(version.raw_version.clone()));
        }
        if let Some(path) = &version.path {
            if path.exists() {
                fs::remove_dir_all(path)?;
            }
        }
        Ok(())
    }

    pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
        let parse = |v: &str| -> Vec<u32> {
            v.split('.')
                .filter_map(|p| {
                    p.chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse()
                        .ok()
                })
                .collect()
        };
        parse(v1).cmp(&parse(v2))
    }
}
