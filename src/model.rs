use crate::config::ReactionsConfig;
use crate::slack::Message;

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
    if msg.has_any_reaction(&reactions.completed) {
        Status::Completed
    } else if msg.has_any_reaction(&reactions.blocked) {
        Status::Blocked
    } else if msg.has_any_reaction(&reactions.taking_a_look) {
        Status::TakingALook
    } else {
        Status::Backlog
    }
}
