use std::str::FromStr;
use sui_types::base_types::ObjectID;

fn parse_object_id(string: &str) -> anyhow::Result<ObjectID> {
    Ok(ObjectID::from_str(string)?)
}

#[derive(clap::Args, Clone, Debug)]
pub struct GasInfo {
    /// ID of the gas object for gas payment
    /// If not provided, a gas object with at least gas-budget value will be selected
    #[arg(name = "gas", long, value_parser = parse_object_id)]
    pub object: Option<ObjectID>,

    /// Maximum amount of gas (in MIST) to use
    #[arg(name = "gas-budget", long, default_value_t = 1000000000)]
    pub budget: u64,
}
