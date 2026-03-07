use crate::config::ReactionsConfig;
use crate::slack::{Message, Reaction};

pub struct TrackedMessage {
    pub channel_id: String,
    pub channel_name: String,
    pub ts: String,
    pub display_name: String,
    pub text: String,
    pub status: usize,
}

pub fn determine_status(msg: &Message, reactions: &ReactionsConfig) -> usize {
    determine_status_from_reactions(&msg.reactions, reactions)
}

pub fn determine_status_from_reactions(reactions: &[Reaction], config: &ReactionsConfig) -> usize {
    let has_any = |names: &[String]| reactions.iter().any(|r| names.contains(&r.name));
    // Check from last to first (last = highest priority). Index 0 is the default.
    for (i, (_name, emojis)) in config.iter().enumerate().rev() {
        if i == 0 {
            break;
        }
        if has_any(emojis) {
            return i;
        }
    }
    0
}
