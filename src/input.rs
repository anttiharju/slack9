pub fn tab_complete_channel(buf: &mut String, channels: &[(String, String)], user_names: &[String]) {
    let (prefix, partial, candidates): (&str, &str, Vec<&str>) = if let Some(rest) = buf.strip_prefix("channel ") {
        (
            "channel ",
            rest.trim_start_matches('#'),
            channels.iter().map(|(_, name)| name.as_str()).collect(),
        )
    } else if let Some(rest) = buf.strip_prefix("c ") {
        (
            "c ",
            rest.trim_start_matches('#'),
            channels.iter().map(|(_, name)| name.as_str()).collect(),
        )
    } else if let Some(rest) = buf.strip_prefix("ping ") {
        ("ping ", rest.trim_start_matches('@'), user_names.iter().map(|s| s.as_str()).collect())
    } else {
        return;
    };

    let matches: Vec<&str> = candidates.iter().copied().filter(|name| name.starts_with(partial)).collect();

    if matches.len() == 1 {
        buf.clear();
        buf.push_str(prefix);
        buf.push_str(matches[0]);
    } else if matches.len() > 1 {
        let mut common = matches[0].to_string();
        for m in &matches[1..] {
            common = common.chars().zip(m.chars()).take_while(|(a, b)| a == b).map(|(a, _)| a).collect();
        }
        if common.len() > partial.len() {
            buf.clear();
            buf.push_str(prefix);
            buf.push_str(&common);
        }
    }
}

pub fn ghost_completion(buf: &str, channels: &[(String, String)], user_names: &[String]) -> String {
    let (partial, candidates): (&str, Vec<&str>) = if let Some(rest) = buf.strip_prefix("channel ") {
        (rest.trim_start_matches('#'), channels.iter().map(|(_, name)| name.as_str()).collect())
    } else if let Some(rest) = buf.strip_prefix("c ") {
        (rest.trim_start_matches('#'), channels.iter().map(|(_, name)| name.as_str()).collect())
    } else if let Some(rest) = buf.strip_prefix("ping ") {
        (rest.trim_start_matches('@'), user_names.iter().map(|s| s.as_str()).collect())
    } else {
        return String::new();
    };

    if partial.is_empty() {
        return String::new();
    }

    let matches: Vec<&str> = candidates.iter().copied().filter(|name| name.starts_with(partial)).collect();

    if matches.len() == 1 {
        matches[0][partial.len()..].to_string()
    } else {
        String::new()
    }
}
