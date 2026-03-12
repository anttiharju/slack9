pub struct TrackedMessage {
    pub channel_id: String,
    pub channel_name: String,
    pub ts: String,
    pub thread_ts: Option<String>,
    pub display_name: String,
    pub text: String,
    pub reaction_emojis: Vec<String>,
    /// Reaction emojis placed by the current user (subset of reaction_emojis).
    pub user_reaction_emojis: Vec<String>,
}

/// Determine the single effective category for a message.
///
/// If the current user has reacted with a category emoji, the highest-priority
/// matching user category wins. Otherwise, the highest-priority category from
/// any reactor wins. Priority order: later in config = higher priority.
pub fn effective_category(msg: &TrackedMessage, categories: &indexmap::IndexMap<String, Vec<String>>) -> Option<String> {
    // Check user's own reactions first (highest priority = last in config → iterate reversed)
    for (name, emojis) in categories.iter().rev() {
        if msg.user_reaction_emojis.iter().any(|e| emojis.contains(e)) {
            return Some(name.clone());
        }
    }
    // Fall back to any reaction (highest priority = last in config)
    for (name, emojis) in categories.iter().rev() {
        if msg.reaction_emojis.iter().any(|e| emojis.contains(e)) {
            return Some(name.clone());
        }
    }
    None
}
