//! Host deny list used when the ad blocker flag is on. Navigations to
//! these hosts are cancelled before the page ever loads. Keeping the
//! rules pure keeps them easy to test; the live gate is read on every
//! navigation attempt so a settings toggle re-gates without rebuilding
//! the view. (Port of the old ad blocker rules.)

use std::sync::atomic::{AtomicBool, Ordering};

pub const DENIED_HOST_SUFFIXES: [&str; 11] = [
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "google-analytics.com",
    "adnxs.com",
    "adsrvr.org",
    "criteo.com",
    "taboola.com",
    "outbrain.com",
    "scorecardresearch.com",
    "quantserve.com",
];

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn blocks_host(host: &str) -> bool {
    let normalized = host.to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    DENIED_HOST_SUFFIXES
        .iter()
        .any(|suffix| is_denied(&normalized, suffix))
}

pub fn blocks_url(url: &str) -> bool {
    match host_of(url) {
        Some(host) => blocks_host(host),
        None => false,
    }
}

fn is_denied(host: &str, suffix: &str) -> bool {
    host == suffix
        || (host.len() > suffix.len()
            && host.ends_with(suffix)
            && host.as_bytes()[host.len() - suffix.len() - 1] == b'.')
}

/// Extracts the host from a URL. Returns `None` for scheme-less strings
/// and empty authorities, mirroring the old parser behavior.
pub fn host_of(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme.is_empty() {
        return None;
    }
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    let host_port = match authority.rsplit_once('@') {
        Some((_, host)) => host,
        None => authority,
    };
    if let Some(open) = host_port.find('[') {
        let end = host_port[open..].find(']')? + open;
        let host = &host_port[open + 1..end];
        return if host.is_empty() { None } else { Some(host) };
    }
    let host = host_port.split(':').next().unwrap_or("");
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_denied_hosts_and_subdomains() {
        for host in [
            "doubleclick.net",
            "ad.doubleclick.net",
            "pagead2.googlesyndication.com",
            "www.google-analytics.com",
            "ADNXS.COM",
        ] {
            assert!(blocks_host(host), "{host}");
        }
    }

    #[test]
    fn allows_everything_else() {
        for host in [
            "",
            "example.com",
            "notdoubleclick.net",
            "evildoubleclick.net.evil.com",
            "google.com",
        ] {
            assert!(!blocks_host(host), "{host}");
        }
    }

    #[test]
    fn urls_resolve_to_hosts_before_the_check() {
        assert!(blocks_url("https://ad.doubleclick.net/dc.js"));
        assert!(blocks_url("https://user@doubleclick.net/x"));
        assert!(blocks_url("http://doubleclick.net:8080/a?b=c"));
        assert!(!blocks_url("https://example.com/page"));
        assert!(!blocks_url("not a url"));
        assert!(!blocks_url("about:blank"));
    }

    #[test]
    fn host_extraction_handles_the_common_shapes() {
        assert_eq!(host_of("https://example.com/path"), Some("example.com"));
        assert_eq!(host_of("https://example.com:8080"), Some("example.com"));
        assert_eq!(host_of("https://user:pw@example.com"), Some("example.com"));
        assert_eq!(host_of("http://[2001:db8::1]/x"), Some("2001:db8::1"));
        assert_eq!(host_of("about:blank"), None);
        assert_eq!(host_of(""), None);
    }
}
