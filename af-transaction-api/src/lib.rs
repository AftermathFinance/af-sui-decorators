use std::{cmp::Ordering, collections::HashMap, sync::Arc};

use anyhow::{anyhow, bail};
use move_core_types::language_storage::StructTag;
use shared_crypto::intent::Intent;
use sui::client_commands::WalletContext;
use sui_keys::keystore::{AccountKeystore, Keystore};
use sui_sdk::{
    rpc_types::{
        ObjectChange, SuiExecutionStatus, SuiTransactionBlockEffects, SuiTransactionBlockEffectsV1,
        SuiTransactionBlockResponse, SuiTransactionBlockResponseOptions,
    },
    SuiClient,
};
use sui_types::{
    base_types::{ObjectID, SuiAddress},
    messages::{ExecuteTransactionRequestType, Transaction, TransactionData},
};

use af_read_api::get_all_coins;

// ==============================================================================
// APIs
// ==============================================================================

#[derive(Clone)]
pub struct SignedTransactionApi {
    pub client: Arc<SuiClient>,
    pub sender: SuiAddress,
    pub keystore: Arc<Keystore>,
}

impl SignedTransactionApi {
    pub async fn from_context(mut context: WalletContext) -> anyhow::Result<Self> {
        Ok(Self {
            client: Arc::new(context.get_client().await?),
            sender: context.active_address()?,
            keystore: Arc::new(context.config.into_inner().keystore),
        })
    }

    pub fn reuse_client(
        client: Arc<SuiClient>,
        mut context: WalletContext,
    ) -> anyhow::Result<Self> {
        let sender = context.active_address()?;
        let keystore = Arc::new(context.config.into_inner().keystore);
        Ok(Self {
            client,
            sender,
            keystore,
        })
    }

    pub async fn sign_and_execute(
        &self,
        tx_data: TransactionData,
        options: SuiTransactionBlockResponseOptions,
    ) -> anyhow::Result<SuiTransactionBlockResponse> {
        sign_and_execute(tx_data, options, self.sender, &self.keystore, &self.client).await
    }

    pub async fn sign_and_execute_with_effects(
        &self,
        tx_data: TransactionData,
    ) -> anyhow::Result<SuiTransactionBlockResponse> {
        sign_and_execute_with_effects(tx_data, self.sender, &self.keystore, &self.client).await
    }

    pub async fn get_coin_amount(
        &self,
        amount: u64,
        coin_type: String,
    ) -> anyhow::Result<ObjectID> {
        get_coin_amount(amount, coin_type, &self.client, &self.keystore, self.sender).await
    }
}

// ==============================================================================
// Functions
// ==============================================================================

pub async fn get_coin_amount(
    amount: u64,
    coin_type: String,
    client: &SuiClient,
    keystore: &Keystore,
    sender: SuiAddress,
) -> anyhow::Result<ObjectID> {
    let coins = get_all_coins(client, sender, coin_type).await?;

    let mut equal = None;
    let mut greater = None;
    for (i, coin) in coins.data.iter().enumerate() {
        match coin.balance.cmp(&amount) {
            Ordering::Equal => {
                equal = Some(i);
                break;
            }
            Ordering::Greater => {
                greater = Some(i);
            }
            _ => {}
        }
    }

    let coin = if let Some(i) = equal {
        coins.data[i].coin_object_id
    } else if let Some(i) = greater {
        let primary = &coins.data[i];
        let tx_data = client
            .transaction_builder()
            .split_coin(
                sender,
                primary.coin_object_id,
                vec![amount, primary.balance - amount],
                None,
                1000,
            )
            .await?;
        sign_and_assert_success(tx_data, sender, keystore, client).await?;
        primary.coin_object_id
    } else {
        ObjectID::ZERO
    };

    Ok(coin)
}

pub async fn sign_and_assert_success(
    tx_data: TransactionData,
    sender: SuiAddress,
    keystore: &Keystore,
    client: &SuiClient,
) -> anyhow::Result<()> {
    let response = sign_and_execute_with_effects(tx_data, sender, keystore, client).await?;
    assert!(
        response.confirmed_local_execution.is_some() && response.confirmed_local_execution.unwrap()
    );
    Ok(())
}

pub async fn sign_and_print_gas_costs(
    tx_data: TransactionData,
    sender: SuiAddress,
    keystore: &Keystore,
    client: &SuiClient,
) -> anyhow::Result<()> {
    let response = sign_and_execute_with_effects(tx_data, sender, keystore, client).await?;
    ensure_transaction_success(&response)?;
    print_gas_costs(&response)
}

pub async fn sign_and_execute_with_effects(
    tx_data: TransactionData,
    sender: SuiAddress,
    keystore: &Keystore,
    client: &SuiClient,
) -> anyhow::Result<SuiTransactionBlockResponse> {
    let options = SuiTransactionBlockResponseOptions::new().with_effects();
    sign_and_execute(tx_data, options, sender, keystore, client).await
}

pub async fn sign_and_execute(
    tx_data: TransactionData,
    options: SuiTransactionBlockResponseOptions,
    sender: SuiAddress,
    keystore: &Keystore,
    client: &SuiClient,
) -> anyhow::Result<SuiTransactionBlockResponse> {
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::sui_transaction())?;

    let transaction =
        Transaction::from_data(tx_data, Intent::sui_transaction(), vec![signature]).verify()?;
    let request_type = Some(ExecuteTransactionRequestType::WaitForLocalExecution);
    Ok(client
        .quorum_driver()
        .execute_transaction_block(transaction, options, request_type)
        .await?)
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

pub fn ensure_transaction_success(response: &SuiTransactionBlockResponse) -> anyhow::Result<()> {
    let effects = get_transaction_effects_v1(response)?;
    if let SuiExecutionStatus::Failure { error } = &effects.status {
        bail!("Transaction failed with status:\n{error}");
    }
    Ok(())
}

pub fn print_transaction_status(response: &SuiTransactionBlockResponse) -> anyhow::Result<()> {
    let effects = get_transaction_effects_v1(response)?;
    println!("Transaction status: {:?}", effects.status);
    Ok(())
}

pub fn get_transaction_effects_v1(
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

// ==============================================================================
// Transaction JSON result parsing
// ==============================================================================

pub struct PublishedObjects {
    pub package_id: ObjectID,
    pub objects: HashMap<String, Vec<CreatedObject>>,
}

impl TryFrom<SuiTransactionBlockResponse> for PublishedObjects {
    type Error = anyhow::Error;

    fn try_from(value: SuiTransactionBlockResponse) -> Result<Self, Self::Error> {
        let mut package = None;
        let mut objects = HashMap::<String, Vec<CreatedObject>>::new();
        for change in object_changes_from_tx_result(value)? {
            match change {
                ObjectChange::Created {
                    object_type,
                    object_id,
                    ..
                } => {
                    let key = object_type.module.to_string() + "::" + object_type.name.as_str();
                    let created = CreatedObject {
                        object_id,
                        object_type,
                    };
                    objects.entry(key).or_default().push(created);
                }
                ObjectChange::Published { package_id, .. } => {
                    package = Some(package_id);
                }
                _ => (),
            }
        }
        Ok(Self {
            package_id: package.ok_or_else(|| anyhow!("Missing package id in tx response"))?,
            objects,
        })
    }
}

pub struct CreatedObject {
    pub object_id: ObjectID,
    pub object_type: StructTag,
}

pub fn object_changes_from_tx_result(
    result: SuiTransactionBlockResponse,
) -> anyhow::Result<Vec<ObjectChange>> {
    result
        .object_changes
        .ok_or_else(|| anyhow!("No object changes in transaction"))
}

//#[cfg(test)]
//mod tests {
//    use super::*;
//
//    #[test]
//    fn it_works() {
//        let result = add(2, 2);
//        assert_eq!(result, 4);
//    }
//}
