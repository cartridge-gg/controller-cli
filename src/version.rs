use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/cartridge-gg/controller-cli/releases/latest";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

#[derive(Serialize, Deserialize)]
struct VersionCache {
    latest_version: String,
    checked_at: u64, // unix timestamp
}

fn cache_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("controller-cli").join(".version-cache"))
}

fn read_cache() -> Option<VersionCache> {
    let path = cache_path()?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_cache(version: &str) {
    if let Some(path) = cache_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let cache = VersionCache {
            latest_version: version.to_string(),
            checked_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        if let Ok(json) = serde_json::to_string(&cache) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn is_cache_fresh(cache: &VersionCache) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(cache.checked_at) < CHECK_INTERVAL.as_secs()
}

/// Parse a version string like "0.1.11" into (major, minor, patch).
fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.strip_prefix('v').unwrap_or(v);
    // Also strip "cli-v" prefix from tag names
    let v = v.strip_prefix("cli-v").unwrap_or(v);
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

/// Returns true if `latest` is newer than `current`.
fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

/// Fetch the latest version from GitHub releases API.
async fn fetch_latest_version() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;

    let resp = client
        .get(GITHUB_RELEASES_URL)
        .header("User-Agent", "controller-cli")
        .send()
        .await
        .ok()?;

    let release: GitHubRelease = resp.json().await.ok()?;
    Some(release.tag_name)
}

/// Check for a newer version. Returns a warning message if one is available.
/// Uses a 24h cache to avoid hitting the API on every invocation.
pub async fn check_for_update() -> Option<String> {
    // First check cache
    if let Some(cache) = read_cache() {
        if is_cache_fresh(&cache) {
            return if is_newer(CURRENT_VERSION, &cache.latest_version) {
                let display = cache
                    .latest_version
                    .strip_prefix("cli-v")
                    .unwrap_or(&cache.latest_version);
                Some(format!(
                    "A new version of controller-cli is available: {CURRENT_VERSION} → {display} \
                     (update: curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash)"
                ))
            } else {
                None
            };
        }
    }

    // Cache is stale or missing — fetch from GitHub
    let tag = fetch_latest_version().await?;
    write_cache(&tag);

    if is_newer(CURRENT_VERSION, &tag) {
        let display = tag.strip_prefix("cli-v").unwrap_or(&tag);
        Some(format!(
            "A new version of controller-cli is available: {CURRENT_VERSION} → {display} \
             (update: curl -fsSL https://raw.githubusercontent.com/cartridge-gg/controller-cli/main/install.sh | bash)"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_plain() {
        assert_eq!(parse_version("0.1.11"), Some((0, 1, 11)));
    }

    #[test]
    fn test_parse_version_with_v_prefix() {
        assert_eq!(parse_version("v0.1.11"), Some((0, 1, 11)));
    }

    #[test]
    fn test_parse_version_with_cli_v_prefix() {
        assert_eq!(parse_version("cli-v0.1.11"), Some((0, 1, 11)));
    }

    #[test]
    fn test_parse_version_invalid() {
        assert_eq!(parse_version("not-a-version"), None);
        assert_eq!(parse_version("1.2"), None);
    }

    #[test]
    fn test_is_newer_patch() {
        assert!(is_newer("0.1.10", "0.1.11"));
        assert!(!is_newer("0.1.11", "0.1.10"));
        assert!(!is_newer("0.1.11", "0.1.11"));
    }

    #[test]
    fn test_is_newer_minor() {
        assert!(is_newer("0.1.11", "0.2.0"));
        assert!(!is_newer("0.2.0", "0.1.99"));
    }

    #[test]
    fn test_is_newer_major() {
        assert!(is_newer("0.9.99", "1.0.0"));
        assert!(!is_newer("1.0.0", "0.9.99"));
    }

    #[test]
    fn test_is_newer_with_tag_prefix() {
        assert!(is_newer("0.1.10", "cli-v0.1.11"));
        assert!(is_newer("0.1.10", "v0.1.11"));
    }

    #[test]
    fn test_is_newer_same_version() {
        assert!(!is_newer("0.1.11", "0.1.11"));
        assert!(!is_newer("0.1.11", "cli-v0.1.11"));
    }

    #[test]
    fn test_cache_freshness() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let fresh = VersionCache {
            latest_version: "0.1.11".to_string(),
            checked_at: now - 3600, // 1 hour ago
        };
        assert!(is_cache_fresh(&fresh));

        let stale = VersionCache {
            latest_version: "0.1.11".to_string(),
            checked_at: now - 90_000, // 25 hours ago
        };
        assert!(!is_cache_fresh(&stale));
    }
}
