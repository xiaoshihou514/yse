use rand::seq::SliceRandom;
use rand::Rng;

const HOT_DOMAINS: &[&str] = &[
    "gmail.com",
    "outlook.com",
    "yahoo.com",
    "protonmail.com",
    "icloud.com",
    "qq.com",
    "163.com",
];

pub fn random_domain() -> &'static str {
    HOT_DOMAINS.choose(&mut rand::thread_rng()).unwrap_or(&"gmail.com")
}

fn random_username_for_domain(domain: &str) -> String {
    let mut rng = rand::thread_rng();
    match domain {
        "qq.com" | "163.com" => {
            let len = rng.gen_range(6..=12);
            (0..len)
                .map(|_| rng.gen_range(b'0'..=b'9') as char)
                .collect()
        }
        _ => {
            let first = random_name_part(&mut rng);
            let last = random_name_part(&mut rng);
            let suffix: String = if rng.gen_bool(0.3) {
                format!("{}", rng.gen_range(10..999))
            } else {
                String::new()
            };
            format!("{}.{}{}", first.to_lowercase(), last.to_lowercase(), suffix)
        }
    }
}

fn random_name_part(rng: &mut impl Rng) -> &'static str {
    const NAMES: &[&str] = &[
        "john", "jane", "alex", "sarah", "mike", "emma", "david", "olivia", "james", "sophia",
        "robert", "isabella", "william", "ava", "richard", "mia", "joseph", "charlotte",
        "thomas", "amelia", "christopher", "harper", "daniel", "evelyn", "matthew", "abigail",
        "anthony", "emily", "mark", "elizabeth", "donald", "sofia", "steven", "avery", "paul",
        "ella", "andrew", "madison", "joshua", "scott", "kevin", "oliver", "brian", "lily",
        "george", "chloe", "edward", "grace", "peter", "zoe",
    ];
    NAMES.choose(rng).unwrap_or(&"user")
}

#[derive(Debug, Clone)]
pub struct DisguisedSender {
    pub from_addr: String,
    pub display_name: String,
}

pub fn disguise() -> DisguisedSender {
    let domain = random_domain();
    let username = random_username_for_domain(domain);
    let display_name = random_display_name();
    DisguisedSender {
        from_addr: format!("{}@{}", username, domain),
        display_name,
    }
}

fn random_display_name() -> String {
    let mut rng = rand::thread_rng();
    let first = capitalized_word(&mut rng);
    let last = capitalized_word(&mut rng);
    format!("{} {}", first, last)
}

fn capitalized_word(rng: &mut impl Rng) -> &'static str {
    const WORDS: &[&str] = &[
        "John", "Jane", "Alex", "Sarah", "Mike", "Emma", "David", "Olivia", "James", "Sophia",
        "Robert", "Isabella", "William", "Ava", "Richard", "Mia", "Joseph", "Charlotte",
        "Thomas", "Amelia", "Christopher", "Harper", "Daniel", "Evelyn", "Matthew", "Abigail",
        "Anthony", "Emily", "Mark", "Elizabeth", "Donald", "Sofia", "Steven", "Avery", "Paul",
        "Ella", "Andrew", "Madison", "Joshua", "Scott", "Kevin", "Oliver", "Brian", "Lily",
        "George", "Chloe", "Edward", "Grace", "Peter", "Zoe",
    ];
    WORDS.choose(rng).unwrap_or(&"User")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disguise_format() {
        for _ in 0..100 {
            let d = disguise();
            assert!(!d.from_addr.is_empty());
            assert!(d.from_addr.contains('@'));
            assert!(!d.display_name.is_empty());
            if d.from_addr.ends_with("@qq.com") {
                let username = d.from_addr.split('@').next().unwrap();
                assert!(username.chars().all(|c| c.is_ascii_digit()));
                assert!(username.len() >= 6 && username.len() <= 12);
            }
        }
    }

    #[test]
    fn test_random_domain() {
        let domain = random_domain();
        assert!(HOT_DOMAINS.contains(&domain));
    }
}
