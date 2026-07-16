use rand::Rng;

/// Parse `name#hash@hostname` into `(name, hash, hostname)`.
pub fn parse_address(addr: &str) -> Option<(&str, &str, &str)> {
    let (local, hostname) = addr.rsplit_once('@')?;
    let (name, hash) = local.split_once('#')?;
    Some((name, hash, hostname))
}

/// Format components into `name#hash@hostname`.
pub fn format_address(name: &str, hash: &str, hostname: &str) -> String {
    format!("{}#{}@{}", name, hash, hostname)
}

/// Get the local machine's hostname.
pub fn local_hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "localhost".to_string())
}

/// Check if `addr` targets this machine.
pub fn is_for_this_machine(addr: &str, our_hostname: &str) -> bool {
    parse_address(addr)
        .map(|(_, _, hostname)| hostname == our_hostname)
        .unwrap_or(false)
}

/// Extract the hostname from an address, or return None.
pub fn extract_hostname(addr: &str) -> Option<&str> {
    addr.rsplit_once('@').map(|(_, h)| h)
}

/// Generate a random 8-character hex hash.
pub fn generate_hash() -> String {
    let mut rng = rand::thread_rng();
    format!("{:08x}", rng.gen::<u32>())
}

/// Extract the name part from an address `name#hash@hostname`, with fallbacks
/// for bare addresses (no `#` or no `@`).
pub fn name_from_addr(addr: &str) -> &str {
    parse_address(addr)
        .map(|(n, _, _)| n)
        .unwrap_or_else(|| addr.split('@').next().unwrap_or(addr))
}

/// Check whether `to_addr` targets the local user identified by `own_name`.
///
/// Matching semantics (matching `imap_ingest::classify`):
/// 1. Parsed name part of `to_addr` equals `own_name`
/// 2. Bare `to_addr` equals `own_name` (for bare `"me"` inputs)
pub fn is_self_addressed(to_addr: &str, own_name: &str) -> bool {
    let to_name = name_from_addr(to_addr);
    to_name == own_name || to_addr == own_name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid() {
        let r = parse_address("bot#a1b2c3d4@my-pc").unwrap();
        assert_eq!(r, ("bot", "a1b2c3d4", "my-pc"));
    }

    #[test]
    fn test_parse_no_hash() {
        assert!(parse_address("bot@host").is_none());
    }

    #[test]
    fn test_parse_no_at() {
        assert!(parse_address("bot#hash").is_none());
    }

    #[test]
    fn test_format_roundtrip() {
        let addr = format_address("echo", "deadbeef", "server-1");
        assert_eq!(addr, "echo#deadbeef@server-1");
        let p = parse_address(&addr).unwrap();
        assert_eq!(p, ("echo", "deadbeef", "server-1"));
    }

    #[test]
    fn test_extract_hostname() {
        assert_eq!(extract_hostname("x#y@host"), Some("host"));
        assert_eq!(extract_hostname("nohash@host"), Some("host"));
        assert_eq!(extract_hostname("noat"), None);
    }

    #[test]
    fn test_is_for_this_machine() {
        assert!(is_for_this_machine("a#b@mybox", "mybox"));
        assert!(!is_for_this_machine("a#b@other", "mybox"));
    }

    #[test]
    fn test_generate_hash_length() {
        let h = generate_hash();
        assert_eq!(h.len(), 8);
    }

    // ── name_from_addr ──────────────────────────────────────────────

    #[test]
    fn name_from_standard_format() {
        assert_eq!(name_from_addr("me#3a0e53f2@SHLPT0103"), "me");
        assert_eq!(name_from_addr("opencode#6b214911@SHLPT0103"), "opencode");
    }

    #[test]
    fn name_from_chinese_plugin() {
        assert_eq!(name_from_addr("复读机#73e1506b@fedora"), "复读机");
        assert_eq!(name_from_addr("文件#4a77200a@fedora"), "文件");
        assert_eq!(name_from_addr("项目经理#bbdc69ff@fedora"), "项目经理");
    }

    #[test]
    fn name_from_bare_me() {
        // Real case: `from_addr = "me"` (bare, no hash)
        assert_eq!(name_from_addr("me"), "me");
    }

    #[test]
    fn name_from_addr_without_hash() {
        // Without `#` → parse_address returns None → fallback to local part
        assert_eq!(name_from_addr("bot@external.com"), "bot");
        assert_eq!(name_from_addr("me@yse.org"), "me");
        assert_eq!(name_from_addr("alice@somewhere"), "alice");
    }

    #[test]
    fn name_from_bare_string() {
        // No `@` at all → fallback returns addr as-is
        assert_eq!(name_from_addr("alice"), "alice");
        assert_eq!(name_from_addr(""), "");
    }

    #[test]
    fn name_from_db_placeholder() {
        // DB stores "N/A" when address is NULL
        assert_eq!(name_from_addr("N/A"), "N/A");
    }

    // ── is_self_addressed ───────────────────────────────────────────
    // own_name is always "me" in the current system

    #[test]
    fn self_self_standard_format() {
        // me → me (full address with hash)
        assert!(is_self_addressed("me#3a0e53f2@SHLPT0103", "me"));
        assert!(is_self_addressed("me#d312588d@24069RA21C", "me"));
        assert!(is_self_addressed("me#f771f9ba@fedora", "me"));
    }

    #[test]
    fn self_self_bare_me() {
        // Real case: `to_addr = "me"` (bare)
        assert!(is_self_addressed("me", "me"));
    }

    #[test]
    fn self_self_addr_without_hash() {
        // Without hash, just `me@host`
        assert!(is_self_addressed("me@yse.org", "me"));
        assert!(is_self_addressed("me@localhost", "me"));
    }

    #[test]
    fn self_remote_plugin_addressed_to_me() {
        // Plugin → me: `to_addr` starts with a plugin name, not "me"
        // These should NOT be self-addressed (they're for the user, but
        // from a plugin's perspective; the sender is the user, so from_addr
        // would be checked separately by the caller)
        assert!(!is_self_addressed("opencode#6b214911@SHLPT0103", "me"));
        assert!(!is_self_addressed("echo#79627d95@SHLPT0103", "me"));
        assert!(!is_self_addressed("文件#4a77200a@fedora", "me"));
    }

    #[test]
    fn self_user_to_plugin_not_self() {
        // me → plugin: `to_addr` is the plugin address
        assert!(!is_self_addressed("opencode#6b214911@SHLPT0103", "me"));
        assert!(!is_self_addressed("项目经理#bbdc69ff@fedora", "me"));
    }

    #[test]
    fn self_external_address() {
        // Messages addressed to external users
        assert!(!is_self_addressed("bot@external.com", "me"));
        assert!(!is_self_addressed("alice@somewhere", "me"));
        assert!(!is_self_addressed("alice", "me"));
    }

    #[test]
    fn self_with_different_own_name() {
        // What if own_name differs? Should still work correctly
        assert!(is_self_addressed("alice#a1b2@host", "alice"));
        assert!(is_self_addressed("alice", "alice"));
        assert!(!is_self_addressed("bob#c3d4@host", "alice"));
        assert!(!is_self_addressed("me#hash@host", "alice"));
    }

    #[test]
    fn self_db_placeholder_not_self() {
        assert!(!is_self_addressed("N/A", "me"));
    }

    #[test]
    fn self_empty_not_self() {
        assert!(!is_self_addressed("", "me"));
    }

    // ── Integration scenario: classify equivalence ──────────────────
    // These verify that is_self_addressed matches the semantics of
    // imap_ingest::classify's for_self check.

    #[test]
    fn self_matches_all_db_patterns() {
        // Real `to_addr` values from the DB that should be self-addressed:
        let self_addrs = [
            "me#3a0e53f2@SHLPT0103",
            "me#5c7b19cf@SHLPT0103",
            "me#f44e55c9@SHLPT0103",
            "me#d312588d@24069RA21C",
            "me#6189b650@24069RA21C",
            "me#7364ae43@24069RA21C",
            "me#0b79b691@24069RA21C",
            "me#e7853bac@24069RA21C",
            "me#7e170d40@SHLPT0103",
            "me#58e398e7@SHLPT0103",
            "me#f771f9ba@fedora",
            "me#93b574aa@fedora",
            "me#c75b6704@fedora",
            "me#b403b2f0@fedora",
            "me#cab5270d@fedora",
            "me#0d82c939@fedora",
            "me#a87c2d7a@fedora",
            "me#af7350c9@fedora",
            "me#5cd33230@fedora",
            "me#b82229a0@fedora",
            "me#18c984c1@fedora",
            "me#3495338c@fedora",
            "me",
        ];
        for addr in &self_addrs {
            assert!(
                is_self_addressed(addr, "me"),
                "expected self-addressed: {addr}"
            );
        }
    }

    #[test]
    fn self_rejects_all_db_plugin_patterns() {
        // Real `to_addr` values from the DB that should NOT be self-addressed:
        let plugin_addrs = [
            "opencode#6b214911@SHLPT0103",
            "opencode#0609fc98@SHLPT0103",
            "opencode#56cb8511@SHLPT0103",
            "opencode#5854d05f@fedora",
            "opencode#820edadf@fedora",
            "echo#0181ff92@SHLPT0103",
            "echo#79627d95@SHLPT0103",
            "复读机#73e1506b@fedora",
            "文件#4a77200a@fedora",
            "项目经理#bbdc69ff@fedora",
            "项目经理#c5bb3a0b@fedora",
            "file-tree#febca5af@SHLPT0103",
        ];
        for addr in &plugin_addrs {
            assert!(
                !is_self_addressed(addr, "me"),
                "expected NOT self-addressed: {addr}"
            );
        }
    }
}
