use std::path::PathBuf;
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::Security::Cryptography::*;
use windows::core::{PCWSTR, w};

const API: &str = "api.github.com";
const LATEST: &str = "/repos/nyedle/ResolutionSwitcher/releases/latest";

pub enum Step {
    Checking,
    UpToDate,
    Found(String),
    Ready(String),
    Failed(String),
}

pub fn asset_name() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "resolution-switcher-arm64.exe"
    } else {
        "resolution-switcher-x64.exe"
    }
}

pub fn sweep() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(exe.with_extension("old"));
    }
}

pub fn look(report: impl Fn(Step)) {
    report(Step::Checking);
    let body = match fetch(API, LATEST) {
        Ok(bytes) => bytes,
        Err(e) => return report(Step::Failed(e)),
    };
    let json: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(e) => return report(Step::Failed(e.to_string())),
    };

    let Some(found) = pick(&json, env!("CARGO_PKG_VERSION"), asset_name()) else {
        return report(Step::UpToDate);
    };
    report(Step::Found(found.version.clone()));

    match install(&found.url, &found.digest) {
        Ok(()) => report(Step::Ready(found.version)),
        Err(e) => report(Step::Failed(e)),
    }
}

pub struct Release {
    pub version: String,
    pub url: String,
    pub digest: String,
}

pub fn pick(release: &serde_json::Value, current: &str, wanted: &str) -> Option<Release> {
    let tag = release.get("tag_name")?.as_str()?;
    let version = tag.trim_start_matches('v').to_string();
    if !newer(current, &version) {
        return None;
    }
    let asset = release
        .get("assets")?
        .as_array()?
        .iter()
        .find(|asset| asset.get("name").and_then(|n| n.as_str()) == Some(wanted))?;

    Some(Release {
        version,
        url: asset.get("browser_download_url")?.as_str()?.to_string(),
        digest: asset
            .get("digest")?
            .as_str()?
            .strip_prefix("sha256:")?
            .to_lowercase(),
    })
}

pub fn newer(current: &str, candidate: &str) -> bool {
    let parts = |text: &str| -> Vec<u32> {
        text.split(['.', '-', '+'])
            .map_while(|piece| piece.parse().ok())
            .collect()
    };
    let (here, there) = (parts(current), parts(candidate));
    if there.is_empty() {
        return false;
    }
    for index in 0..here.len().max(there.len()) {
        let a = here.get(index).copied().unwrap_or(0);
        let b = there.get(index).copied().unwrap_or(0);
        if a != b {
            return b > a;
        }
    }
    false
}

fn install(url: &str, expected: &str) -> Result<(), String> {
    let (host, path) = split(url).ok_or("that download link makes no sense")?;
    let bytes = fetch(&host, &path)?;

    let got = sha256(&bytes)?;
    if got != expected {
        return Err("the download did not match its published checksum".into());
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    swap(&exe, &bytes)
}

fn sha256(bytes: &[u8]) -> Result<String, String> {
    unsafe {
        let mut algorithm = BCRYPT_ALG_HANDLE::default();
        BCryptOpenAlgorithmProvider(
            &mut algorithm,
            BCRYPT_SHA256_ALGORITHM,
            None,
            Default::default(),
        )
        .ok()
        .map_err(|_| "no SHA-256 on this machine".to_string())?;

        let mut hash = BCRYPT_HASH_HANDLE::default();
        BCryptCreateHash(algorithm, &mut hash, None, None, Default::default())
            .ok()
            .map_err(|_| "could not start hashing".to_string())?;

        let result = (|| {
            BCryptHashData(hash, bytes, 0)
                .ok()
                .map_err(|_| "could not hash the download".to_string())?;
            let mut digest = [0u8; 32];
            BCryptFinishHash(hash, &mut digest, 0)
                .ok()
                .map_err(|_| "could not finish hashing".to_string())?;
            Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
        })();

        let _ = BCryptDestroyHash(hash);
        let _ = BCryptCloseAlgorithmProvider(algorithm, 0);
        result
    }
}

fn swap(exe: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 512 * 1024 || !bytes.starts_with(b"MZ") {
        return Err("the download was not a Windows program".into());
    }

    let fresh: PathBuf = exe.with_extension("new");
    let stale: PathBuf = exe.with_extension("old");

    std::fs::write(&fresh, bytes).map_err(|_| writable(exe))?;
    let _ = std::fs::remove_file(&stale);
    if let Err(e) = std::fs::rename(exe, &stale) {
        let _ = std::fs::remove_file(&fresh);
        return Err(format!("{}: {e}", writable(exe)));
    }
    if let Err(e) = std::fs::rename(&fresh, exe) {
        let _ = std::fs::rename(&stale, exe);
        return Err(e.to_string());
    }
    Ok(())
}

fn writable(exe: &std::path::Path) -> String {
    format!(
        "no permission to write to {}",
        exe.parent().unwrap_or(exe).display()
    )
}

fn split(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://")?;
    let cut = rest.find('/')?;
    let host = &rest[..cut];
    if !ours(host) {
        return None;
    }
    Some((host.to_string(), rest[cut..].to_string()))
}

fn ours(host: &str) -> bool {
    let host = host.to_lowercase();
    host == "github.com"
        || host == "api.github.com"
        || host == "objects.githubusercontent.com"
        || host.ends_with(".github.com")
        || host.ends_with(".githubusercontent.com")
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

struct Handle(*mut std::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) }.ok();
        }
    }
}

fn fetch(host: &str, path: &str) -> Result<Vec<u8>, String> {
    unsafe {
        let session = Handle(WinHttpOpen(
            w!("ResolutionSwitcher"),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            None,
            None,
            0,
        ));
        if session.0.is_null() {
            return Err("could not start a connection".into());
        }

        let host_wide = wide(host);
        let connection = Handle(WinHttpConnect(
            session.0,
            PCWSTR(host_wide.as_ptr()),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        ));
        if connection.0.is_null() {
            return Err(format!("could not reach {host}"));
        }

        let path_wide = wide(path);
        let request = Handle(WinHttpOpenRequest(
            connection.0,
            w!("GET"),
            PCWSTR(path_wide.as_ptr()),
            None,
            None,
            std::ptr::null_mut(),
            WINHTTP_FLAG_SECURE,
        ));
        if request.0.is_null() {
            return Err("could not build the request".into());
        }

        let headers = wide("Accept: application/vnd.github+json\r\n");
        WinHttpSendRequest(
            request.0,
            Some(&headers[..headers.len() - 1]),
            None,
            0,
            0,
            0,
        )
        .map_err(|_| "could not send the request".to_string())?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut())
            .map_err(|_| format!("{host} did not answer"))?;

        let mut status = 0u32;
        let mut size = size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            None,
            Some((&raw mut status).cast()),
            &mut size,
            std::ptr::null_mut(),
        )
        .map_err(|_| "no reply status".to_string())?;
        if status == 404 {
            return Err("no releases published yet".into());
        }
        if !(200..300).contains(&status) {
            return Err(format!("{host} answered {status}"));
        }

        let mut body = Vec::new();
        loop {
            let mut waiting = 0u32;
            WinHttpQueryDataAvailable(request.0, &mut waiting)
                .map_err(|_| "the download stalled".to_string())?;
            if waiting == 0 {
                break;
            }
            let start = body.len();
            body.resize(start + waiting as usize, 0);
            let mut read = 0u32;
            WinHttpReadData(
                request.0,
                body[start..].as_mut_ptr().cast(),
                waiting,
                &mut read,
            )
            .map_err(|_| "the download broke off".to_string())?;
            body.truncate(start + read as usize);
            if read == 0 {
                break;
            }
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert!(newer("2.0.0", "2.0.1"));
        assert!(newer("2.0.0", "2.1.0"));
        assert!(newer("2.9.0", "2.10.0"));
        assert!(!newer("2.0.0", "2.0.0"));
        assert!(!newer("2.1.0", "2.0.9"));
        assert!(!newer("2.0.0", "not-a-version"));
        assert!(newer("2.0", "2.0.1"));
    }

    #[test]
    fn a_release_yields_the_asset_and_its_digest_for_this_machine() {
        let release = serde_json::json!({
            "tag_name": "v2.5.0",
            "assets": [
                {"name": "ResolutionSwitcher-Setup-x64.exe",
                 "browser_download_url": "https://x/setup", "digest": "sha256:aaaa"},
                {"name": "resolution-switcher-x64.exe",
                 "browser_download_url": "https://x/x64", "digest": "sha256:BA7816BF"},
                {"name": "resolution-switcher-arm64.exe",
                 "browser_download_url": "https://x/arm64", "digest": "sha256:c0ffee"}
            ]
        });

        let found = pick(&release, "2.0.0", "resolution-switcher-arm64.exe").expect("newer");
        assert_eq!(found.version, "2.5.0");
        assert_eq!(found.url, "https://x/arm64");
        assert_eq!(found.digest, "c0ffee");

        let cased = pick(&release, "2.0.0", "resolution-switcher-x64.exe").expect("newer");
        assert_eq!(cased.digest, "ba7816bf");

        assert!(pick(&release, "2.5.0", "resolution-switcher-x64.exe").is_none());
        assert!(pick(&release, "2.0.0", "resolution-switcher-riscv.exe").is_none());
        assert!(pick(&serde_json::json!({}), "1.0.0", "anything").is_none());
    }

    #[test]
    fn an_asset_with_no_digest_is_refused_rather_than_trusted() {
        let release = serde_json::json!({
            "tag_name": "v9.0.0",
            "assets": [
                {"name": "resolution-switcher-x64.exe", "browser_download_url": "https://x/x64"}
            ]
        });
        assert!(pick(&release, "1.0.0", "resolution-switcher-x64.exe").is_none());
    }

    #[test]
    fn swapping_keeps_the_old_exe_and_refuses_rubbish() {
        let folder = std::env::temp_dir().join("rs-swap-test");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let exe = folder.join("app.exe");
        std::fs::write(&exe, b"old contents").unwrap();

        let mut program = b"MZ".to_vec();
        program.resize(600 * 1024, 7);
        swap(&exe, &program).unwrap();

        assert_eq!(std::fs::read(&exe).unwrap(), program);
        assert_eq!(
            std::fs::read(exe.with_extension("old")).unwrap(),
            b"old contents"
        );
        assert!(!exe.with_extension("new").exists());

        let before = std::fs::read(&exe).unwrap();
        assert!(swap(&exe, b"not a program").is_err());
        assert!(swap(&exe, &vec![0u8; 600 * 1024]).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), before);

        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn hashing_matches_the_known_vectors() {
        assert_eq!(
            sha256(b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256(b"").unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn only_github_over_https_is_downloaded_from() {
        assert_eq!(
            split("https://github.com/a/b/releases/download/v1/x.exe"),
            Some((
                "github.com".into(),
                "/a/b/releases/download/v1/x.exe".into()
            ))
        );
        assert!(split("https://objects.githubusercontent.com/x").is_some());

        assert!(split("http://github.com/x").is_none());
        assert!(split("https://evil.example/x").is_none());
        assert!(split("https://github.com.evil.example/x").is_none());
        assert!(split("https://evil.example/github.com/x").is_none());
        assert!(split("nonsense").is_none());
    }
}
