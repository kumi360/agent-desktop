mod bridge;
mod child;
mod endpoint;
mod lifecycle;
mod narration;
mod spawn;

pub(crate) use child::entry_from_env;
pub(crate) use spawn::update;
