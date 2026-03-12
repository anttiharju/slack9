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
