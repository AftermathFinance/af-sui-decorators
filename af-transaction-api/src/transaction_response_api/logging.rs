use anyhow::bail;
use sui_sdk::rpc_types::{
        SuiExecutionStatus,
        SuiTransactionBlockEffects,
        SuiTransactionBlockResponse,
        SuiTransactionBlockEffectsV1,
};

fn get_transaction_effects_v1(
    response: &SuiTransactionBlockResponse,
) -> anyhow::Result<&SuiTransactionBlockEffectsV1> {
    if let Some(SuiTransactionBlockEffects::V1(effects)) = &response.effects {
        Ok(effects)
    } else {
        Err(anyhow::anyhow!(
            "No transaction effects in response {response:?}"
        ))
    }
}

pub fn print_effects(response: &SuiTransactionBlockResponse) -> anyhow::Result<()> {
    println!(
        "Confirmed local execution: {:?}",
        response.confirmed_local_execution.unwrap()
    );

    if let Some(SuiTransactionBlockEffects::V1(effects)) = &response.effects {
        if let SuiExecutionStatus::Failure { error } = &effects.status {
            bail!("Transaction failed with status:\n{error}");
        }

        println!("{:#?}", effects.gas_used);
        if !effects.created.is_empty() {
            println!("Created:");
            for created in effects.created.iter() {
                println!("{:#?}", created);
            }
        }
    } else {
        println!("No transaction effects")
    }

    Ok(())
}

pub fn print_gas_costs(response: &SuiTransactionBlockResponse) -> anyhow::Result<()> {
    let effects = get_transaction_effects_v1(response)?;
    println!("{:?}", effects.gas_used);
    Ok(())
}

pub fn print_transaction_status(response: &SuiTransactionBlockResponse) -> anyhow::Result<()> {
    let effects = get_transaction_effects_v1(response)?;
    println!("Transaction status: {:?}", effects.status);
    Ok(())
}
