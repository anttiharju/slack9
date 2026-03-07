pub fn tab_complete_channel(buf: &mut String, channels: &[(String, String)]) {
    let (prefix, partial) = if let Some(rest) = buf.strip_prefix("channel ") {
        ("channel ", rest.trim_start_matches('#'))
    } else if let Some(rest) = buf.strip_prefix("c ") {
        ("c ", rest.trim_start_matches('#'))
    } else {
        return;
    };

    let matches: Vec<&str> = channels
        .iter()
        .map(|(_, name)| name.as_str())
        .filter(|name| name.starts_with(partial))
        .collect();

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

pub fn ghost_completion(buf: &str, channels: &[(String, String)]) -> String {
    let partial = if let Some(rest) = buf.strip_prefix("channel ") {
        rest.trim_start_matches('#')
    } else if let Some(rest) = buf.strip_prefix("c ") {
        rest.trim_start_matches('#')
    } else {
        return String::new();
    };

    if partial.is_empty() {
        return String::new();
    }

    let matches: Vec<&str> = channels
        .iter()
        .map(|(_, name)| name.as_str())
        .filter(|name| name.starts_with(partial))
        .collect();

    if matches.len() == 1 {
        matches[0][partial.len()..].to_string()
    } else {
        String::new()
    }
}
