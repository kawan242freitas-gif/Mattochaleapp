use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{fs::File, io::{Read, Write}, path::{Path, PathBuf}};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASE_API: &str = "https://api.github.com/repos/kawan242freitas-gif/Mattochaleapp/releases/latest";
const INSTALLER_PREFIX: &str = "MattoChales-Setup-";

#[derive(Clone, Debug)]
pub struct UpdateInfo { pub version: String, pub url: String, pub sha256: String }

#[derive(Deserialize)]
struct Release { tag_name: String, assets: Vec<Asset> }
#[derive(Deserialize)]
struct Asset { name: String, browser_download_url: String, digest: Option<String> }

pub fn latest() -> Result<Option<UpdateInfo>> {
    let release = reqwest::blocking::Client::builder().user_agent("MattoChales-Updater").build()?
        .get(RELEASE_API).send()?.error_for_status()?.json::<Release>()?;
    let version = release.tag_name.trim_start_matches('v').to_owned();
    if !is_newer(&version, CURRENT_VERSION) { return Ok(None); }
    let asset = release.assets.into_iter().find(|asset| asset.name.starts_with(INSTALLER_PREFIX) && asset.name.ends_with(".exe"))
        .context("A Release nova não contém o instalador do Windows")?;
    let sha256 = asset.digest.unwrap_or_default().strip_prefix("sha256:").unwrap_or("").to_owned();
    if sha256.len() != 64 { bail!("A Release não publicou o hash SHA-256 do instalador"); }
    Ok(Some(UpdateInfo { version, url: asset.browser_download_url, sha256 }))
}

fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| v.split('.').map(|n| n.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let mut a = parse(candidate); let mut b = parse(current); a.resize(3, 0); b.resize(3, 0); a > b
}

pub fn download(info: &UpdateInfo, mut progress: impl FnMut(u8)) -> Result<PathBuf> {
    let response = reqwest::blocking::Client::builder().user_agent("MattoChales-Updater").build()?
        .get(&info.url).send()?.error_for_status()?;
    let total = response.content_length().unwrap_or(0);
    let path = std::env::temp_dir().join(format!("{INSTALLER_PREFIX}{}.exe", info.version));
    let mut file = File::create(&path)?; let mut response = response;
    let mut buffer = [0u8; 64 * 1024]; let mut received = 0u64;
    loop { let count = response.read(&mut buffer)?; if count == 0 { break; }
        file.write_all(&buffer[..count])?; received += count as u64;
        if total > 0 { progress(((received.saturating_mul(100) / total).min(100)) as u8); }
    }
    file.sync_all()?;
    drop(file);
    let actual = match sha256_file(&path) {
        Ok(hash) => hash,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(error).context("Não foi possível verificar o SHA-256 do instalador");
        }
    };
    if actual != info.sha256 {
        let _ = std::fs::remove_file(&path);
        bail!("O hash do instalador não corresponde ao publicado no GitHub");
    }
    Ok(path)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }
    Ok(context
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
