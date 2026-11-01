//! Versiecheck: vraagt de nieuwste release op GitHub op en vergelijkt het versienummer.
//! Enkel een melding met een link; installeren doet 1.0 niet.
use serde::Serialize;
use std::time::Duration;

pub const RELEASES_API: &str = "https://api.github.com/repos/Safix94/netjes/releases/latest";

#[derive(Serialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum UpdateState {
    Available,
    Latest,
    Error,
}

#[derive(Serialize, Debug, PartialEq, Eq, Clone)]
pub struct UpdateInfo {
    pub state: UpdateState,
    pub version: Option<String>,
}

/// "v1.2.3" of "1.2.3" naar (1, 2, 3); ontbrekende delen tellen als 0, rommel als None.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim();
    let text = text.strip_prefix('v').or_else(|| text.strip_prefix('V')).unwrap_or(text);
    if text.is_empty() {
        return None;
    }
    let mut parts = [0u64; 3];
    for (i, piece) in text.split('.').enumerate() {
        if i >= 3 {
            break;
        }
        parts[i] = piece.parse().ok()?;
    }
    Some((parts[0], parts[1], parts[2]))
}

/// Is de release met `tag` nieuwer dan de draaiende versie `current`?
pub fn is_newer(tag: &str, current: &str) -> bool {
    match (parse_version(tag), parse_version(current)) {
        (Some(release), Some(running)) => release > running,
        _ => false,
    }
}

/// Vertaalt het antwoord van de GitHub-API (of het ontbreken ervan) naar een UpdateInfo.
pub fn classify(tag: Option<&str>, current: &str) -> UpdateInfo {
    match tag {
        Some(tag) if is_newer(tag, current) => UpdateInfo {
            state: UpdateState::Available,
            version: Some(tag.trim().trim_start_matches(['v', 'V']).to_string()),
        },
        _ => UpdateInfo { state: UpdateState::Latest, version: None },
    }
}

/// Haalt de nieuwste release op. Geen release (404) telt als "nieuwste"; geen netwerk als fout.
pub fn check(current: &str) -> UpdateInfo {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .user_agent(&format!("Netjes/{current}"))
        .build();
    match agent.get(RELEASES_API).set("Accept", "application/vnd.github+json").call() {
        Ok(response) => match response.into_json::<serde_json::Value>() {
            Ok(body) => classify(body.get("tag_name").and_then(|v| v.as_str()), current),
            Err(_) => UpdateInfo { state: UpdateState::Error, version: None },
        },
        Err(ureq::Error::Status(404, _)) => classify(None, current),
        Err(_) => UpdateInfo { state: UpdateState::Error, version: None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags_with_and_without_v() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_version("v2.1"), Some((2, 1, 0)));
        assert_eq!(parse_version("latest"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn newer_only_when_the_tag_is_strictly_higher() {
        assert!(is_newer("v1.0.1", "1.0.0"));
        assert!(is_newer("v1.1.0", "1.0.9"));
        assert!(is_newer("v2.0.0", "1.9.9"));
        assert!(!is_newer("v1.0.0", "1.0.0"));
        assert!(!is_newer("v0.9.0", "1.0.0"));
        assert!(!is_newer("onzin", "1.0.0"));
    }

    #[test]
    fn classifies_release_answers() {
        assert_eq!(
            classify(Some("v1.1.0"), "1.0.0"),
            UpdateInfo { state: UpdateState::Available, version: Some("1.1.0".into()) }
        );
        assert_eq!(classify(Some("v1.0.0"), "1.0.0"), UpdateInfo { state: UpdateState::Latest, version: None });
        // Nog geen release: dan heb je de nieuwste.
        assert_eq!(classify(None, "1.0.0"), UpdateInfo { state: UpdateState::Latest, version: None });
    }
}
