use semver::Version;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/LaurentHouel/rs-DiskAnalyzer/releases/latest";

pub struct Release {
    pub version: String,
    pub url: String,
}

pub type CheckResult = Result<Option<Release>, String>;

/// Starts a background GitHub release check and returns its result channel.
///
/// # Parameters
/// - `ctx`: egui context to request a repaint after the check completes.
///
/// # Returns
/// A receiver that yields the release-check result once available.
pub fn start_check(ctx: egui::Context) -> Receiver<CheckResult> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        if tx.send(check_for_update()).is_ok() {
            ctx.request_repaint();
        }
    });
    rx
}

/// Fetches the latest GitHub release and compares it with the running application version.
///
/// # Parameters
/// None.
///
/// # Returns
/// A newer release when available, `None` when current, or an error describing the failed check.
fn check_for_update() -> CheckResult {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(5))
        .timeout_write(Duration::from_secs(5))
        .build();
    let response = agent
        .get(LATEST_RELEASE_URL)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "rs-DiskAnalyzer")
        .call()
        .map_err(|err| err.to_string())?;
    let body = response.into_string().map_err(|err| err.to_string())?;
    let release: serde_json::Value =
        serde_json::from_str(&body).map_err(|err| format!("Invalid GitHub response: {err}"))?;
    let tag = release
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "GitHub release has no tag_name".to_owned())?;
    let url = release
        .get("html_url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "GitHub release has no html_url".to_owned())?;

    let version = parse_release_version(tag)?;
    let current = Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is valid");
    if version > current {
        Ok(Some(Release {
            version: version.to_string(),
            url: url.to_owned(),
        }))
    } else {
        Ok(None)
    }
}

/// Parses a release tag as a semantic version, accepting an optional leading `v`.
///
/// # Parameters
/// - `tag`: Version tag returned by the release service.
///
/// # Returns
/// The parsed semantic version, or a descriptive parsing error.
fn parse_release_version(tag: &str) -> Result<Version, String> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|err| format!("Invalid release version {tag:?}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Verifies that release tags with and without a `v` prefix compare semantically.
    ///
    /// # Parameters
    /// None.
    ///
    /// # Returns
    /// Nothing; assertions fail if parsing or version ordering is incorrect.
    fn compares_release_tags_semantically() {
        let current = Version::parse("0.1.3").unwrap();
        assert!(parse_release_version("v0.1.4").unwrap() > current);
        assert!(parse_release_version("0.1.3").unwrap() <= current);
        assert!(parse_release_version("v0.1.2").unwrap() < current);
    }
}
