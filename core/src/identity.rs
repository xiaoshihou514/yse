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
}
