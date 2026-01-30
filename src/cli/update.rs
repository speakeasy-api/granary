use std::path::PathBuf;
use std::process::Command;

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::{GranaryError, Result};

const GITHUB_REPO: &str = "speakeasy-api/granary";
const CACHE_TTL_HOURS: i64 = 24;

#[derive(Deserialize, Clone)]
struct GitHubRelease {
    tag_name: String,
    prerelease: bool,
}

#[derive(Serialize, Deserialize)]
struct UpdateCache {
    last_check: DateTime<Utc>,
    latest_version: String,
    #[serde(default)]
    latest_prerelease: Option<String>,
}

/// Get cache file path (~/.granary/update-check.json)
fn cache_path() -> Option<PathBuf> {
    #[cfg(unix)]
    let home = std::env::var("HOME").ok();

    #[cfg(windows)]
    let home = std::env::var("USERPROFILE").ok();

    home.map(|h| PathBuf::from(h).join(".granary").join("update-check.json"))
}

/// Read cached update info (if fresh, <24h old)
fn read_cache() -> Option<UpdateCache> {
    let path = cache_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let cache: UpdateCache = serde_json::from_str(&content).ok()?;

    // Check if cache is still fresh
    let age = Utc::now().signed_duration_since(cache.last_check);
    if age.num_hours() < CACHE_TTL_HOURS {
        Some(cache)
    } else {
        None
    }
}

/// Write update cache
fn write_cache(latest_stable: &str, latest_prerelease: Option<&str>) -> Result<()> {
    let Some(path) = cache_path() else {
        return Ok(()); // Silently skip if no home dir
    };

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let cache = UpdateCache {
        last_check: Utc::now(),
        latest_version: latest_stable.to_string(),
        latest_prerelease: latest_prerelease.map(|s| s.to_string()),
    };

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Version info from GitHub releases
struct VersionInfo {
    latest_stable: String,
    latest_prerelease: Option<String>,
}

/// Fetch all releases from GitHub API
async fn fetch_releases() -> Result<Vec<GitHubRelease>> {
    let url = format!("https://api.github.com/repos/{}/releases", GITHUB_REPO);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "granary-cli")
        .send()
        .await
        .map_err(|e| GranaryError::Network(e.to_string()))?;

    if !response.status().is_success() {
        return Err(GranaryError::Network(format!(
            "GitHub API returned status {}",
            response.status()
        )));
    }

    let releases: Vec<GitHubRelease> = response
        .json()
        .await
        .map_err(|e| GranaryError::Network(e.to_string()))?;

    Ok(releases)
}

/// Strip 'v' prefix from version string
fn strip_v_prefix(version: &str) -> &str {
    version.strip_prefix('v').unwrap_or(version)
}

/// Fetch version info from GitHub (latest stable and optionally latest prerelease)
async fn fetch_version_info() -> Result<VersionInfo> {
    let releases = fetch_releases().await?;

    // Find latest stable (first non-prerelease)
    let latest_stable = releases
        .iter()
        .find(|r| !r.prerelease)
        .map(|r| strip_v_prefix(&r.tag_name).to_string())
        .ok_or_else(|| GranaryError::Network("No stable releases found".to_string()))?;

    // Find latest prerelease (first prerelease)
    let latest_prerelease = releases
        .iter()
        .find(|r| r.prerelease)
        .map(|r| strip_v_prefix(&r.tag_name).to_string());

    Ok(VersionInfo {
        latest_stable,
        latest_prerelease,
    })
}

/// Compare two version strings using semver
fn is_newer_version(current: &str, latest: &str) -> bool {
    match (Version::parse(current), Version::parse(latest)) {
        (Ok(c), Ok(l)) => l > c,
        _ => latest > current, // Fallback to string comparison
    }
}

/// Check for update (fetches from GitHub and updates cache)
pub async fn check_for_update() -> Result<Option<String>> {
    let current = env!("CARGO_PKG_VERSION");
    let info = fetch_version_info().await?;

    // Update cache
    let _ = write_cache(&info.latest_stable, info.latest_prerelease.as_deref());

    if is_newer_version(current, &info.latest_stable) {
        Ok(Some(info.latest_stable))
    } else {
        Ok(None)
    }
}

/// Check for update using cache only (for version display)
pub fn check_for_update_cached() -> Option<String> {
    let current = env!("CARGO_PKG_VERSION");
    let cache = read_cache()?;

    if is_newer_version(current, &cache.latest_version) {
        Some(cache.latest_version)
    } else {
        None
    }
}

/// Get version string with update notice for clap
pub fn version_with_update_notice() -> &'static str {
    let version = env!("CARGO_PKG_VERSION");

    if let Some(latest) = check_for_update_cached() {
        let s = format!(
            "{}\nUpdate available: {} (run `granary update` to install)",
            version, latest
        );
        Box::leak(s.into_boxed_str())
    } else {
        version
    }
}

/// Run the install script to perform the update
fn run_install_script(version: Option<&str>) -> Result<()> {
    #[cfg(unix)]
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg("curl -sSfL https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.sh | sh");

        // Set GRANARY_VERSION env var if a specific version is requested
        if let Some(v) = version {
            cmd.env("GRANARY_VERSION", v);
        }

        let status = cmd
            .status()
            .map_err(|e| GranaryError::Update(format!("Failed to run install script: {}", e)))?;

        if !status.success() {
            return Err(GranaryError::Update("Install script failed".to_string()));
        }
    }

    #[cfg(windows)]
    {
        let script = if let Some(v) = version {
            format!(
                "$env:GRANARY_VERSION='{}'; irm https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.ps1 | iex",
                v
            )
        } else {
            "irm https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.ps1 | iex"
                .to_string()
        };

        let status = Command::new("powershell")
            .arg("-Command")
            .arg(&script)
            .status()
            .map_err(|e| GranaryError::Update(format!("Failed to run install script: {}", e)))?;

        if !status.success() {
            return Err(GranaryError::Update("Install script failed".to_string()));
        }
    }

    Ok(())
}

/// Main update command handler
pub async fn update(check_only: bool, target_version: Option<String>) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");

    // If a specific version is requested, install it directly
    if let Some(ref version) = target_version {
        println!("Installing granary {}...", version);
        println!();

        run_install_script(Some(version))?;

        println!();
        println!("Successfully installed granary {}!", version);
        return Ok(());
    }

    println!("Checking for updates...");

    let info = match fetch_version_info().await {
        Ok(v) => v,
        Err(e) => {
            return Err(GranaryError::Update(format!(
                "Failed to check for updates: {}",
                e
            )));
        }
    };

    // Update cache
    let _ = write_cache(&info.latest_stable, info.latest_prerelease.as_deref());

    let has_stable_update = is_newer_version(current, &info.latest_stable);

    // Check if there's a prerelease newer than the latest stable
    let newer_prerelease = info.latest_prerelease.as_ref().and_then(|pre| {
        if is_newer_version(&info.latest_stable, pre) {
            Some(pre.clone())
        } else {
            None
        }
    });

    if !has_stable_update {
        println!("granary {} is the latest stable version", current);

        // Show prerelease info if available and newer
        if let Some(pre) = &newer_prerelease {
            println!();
            println!("Pre-release available: {}", pre);
            println!("Install with: granary update --to={}", pre);
        }

        return Ok(());
    }

    if check_only {
        println!("Current version: {}", current);
        println!("Latest version:  {}", info.latest_stable);

        // Show prerelease info if available and newer
        if let Some(pre) = &newer_prerelease {
            println!();
            println!("Pre-release available: {}", pre);
            println!("Install with: granary update --to={}", pre);
        }

        println!();
        println!("Run `granary update` to install the latest stable version.");
        return Ok(());
    }

    println!("Updating granary {} → {}...", current, info.latest_stable);
    println!();

    run_install_script(None)?;

    println!();
    println!("Successfully updated to granary {}!", info.latest_stable);

    // Inform about prerelease if available
    if let Some(pre) = &newer_prerelease {
        println!();
        println!("Pre-release {} is also available.", pre);
        println!("Install with: granary update --to={}", pre);
    }

    Ok(())
}
