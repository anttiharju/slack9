use crate::config::ReactionsConfig;
use crate::slack::{Message, Reaction};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Backlog,
    TakingALook,
    Blocked,
    Completed,
}

pub struct TrackedMessage {
    pub channel_id: String,
    pub channel_name: String,
    pub ts: String,
    pub display_name: String,
    pub text: String,
    pub status: Status,
}

pub fn determine_status(msg: &Message, reactions: &ReactionsConfig) -> Status {
    determine_status_from_reactions(&msg.reactions, reactions)
}

pub fn determine_status_from_reactions(reactions: &[Reaction], config: &ReactionsConfig) -> Status {
    let has_any = |names: &[String]| reactions.iter().any(|r| names.contains(&r.name));
    if has_any(&config.completed) {
        Status::Completed
    } else if has_any(&config.blocked) {
        Status::Blocked
    } else if has_any(&config.taking_a_look) {
        Status::TakingALook
    } else {
        Status::Backlog
    }
}
