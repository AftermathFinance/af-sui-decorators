use sui_sdk::{json::SuiJsonValue, rpc_types::SuiTypeTag};
use sui_types::base_types::ObjectID;

pub struct MoveCallArgs {
    pub package: ObjectID,
    pub module: &'static str,
    pub function: &'static str,
    pub type_args: Vec<SuiTypeTag>,
    pub call_args: Vec<SuiJsonValue>,
}

pub trait TryIntoMoveCallArgs<C> {
    fn try_into_args(self, config: &C) -> anyhow::Result<MoveCallArgs>;
}
