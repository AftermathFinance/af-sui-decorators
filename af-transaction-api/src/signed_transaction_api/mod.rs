use std::{cmp::Ordering, sync::Arc};

use anyhow::bail;
use shared_crypto::intent::Intent;
use sui_keys::keystore::{AccountKeystore, Keystore};
use sui_sdk::{
    rpc_types::{SuiTransactionBlockResponse, SuiTransactionBlockResponseOptions},
    wallet_context::WalletContext,
    SuiClient,
};
use sui_transaction_builder::TransactionBuilder;
use sui_types::{
    base_types::{ObjectID, SuiAddress},
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{Transaction, TransactionData}, coin::Coin,
};

use af_read_api::{get_all_coins, ReadObject};
use af_types::{
    gas_info::GasInfo,
    move_call_args::{MoveCallArgs, TryIntoMoveCallArgs},
};

#[derive(Clone)]
pub struct SignedTransactionCaller<C> {
    pub api: SignedTransactionApi,
    pub config: C,
}

impl<C> SignedTransactionCaller<C> {
    pub async fn new(context: WalletContext, config: C) -> anyhow::Result<Self> {
        let api = SignedTransactionApi::from_context(context).await?;
        Ok(Self { api, config })
    }

    pub async fn call_with_effects<T: TryIntoMoveCallArgs<C>>(
        &self,
        args: T,
        gas: GasInfo,
    ) -> anyhow::Result<SuiTransactionBlockResponse> {
        self.api
            .sign_and_execute_with_effects(self.tx_data(args, gas).await?)
            .await
    }

    pub async fn call<T: TryIntoMoveCallArgs<C>>(
        &self,
        args: T,
        gas: GasInfo,
        options: SuiTransactionBlockResponseOptions,
    ) -> anyhow::Result<SuiTransactionBlockResponse> {
        self.api
            .sign_and_execute(self.tx_data(args, gas).await?, options)
            .await
    }

    async fn tx_data<T: TryIntoMoveCallArgs<C>>(
        &self,
        args: T,
        gas: GasInfo,
    ) -> anyhow::Result<TransactionData> {
        let builder = SignedTransactionBuilder {
            config: &self.config,
            builder: self.api.client.transaction_builder(),
            sender: self.api.sender,
            gas,
        };
        builder.call(args).await
    }
}

pub struct SignedTransactionBuilder<'a, C> {
    config: &'a C,
    sender: SuiAddress,
    gas: GasInfo,
    builder: &'a TransactionBuilder,
}

impl<'a, C> SignedTransactionBuilder<'a, C> {
    async fn call<T: TryIntoMoveCallArgs<C>>(&self, args: T) -> anyhow::Result<TransactionData> {
        let MoveCallArgs {
            package,
            module,
            function,
            type_args,
            call_args,
        } = args.try_into_args(self.config)?;
        self.builder
            .move_call(
                self.sender,
                package,
                module,
                function,
                type_args,
                call_args,
                self.gas.object,
                self.gas.budget,
            )
            .await
    }
}

#[derive(Clone)]
pub struct SignedTransactionApi {
    pub client: Arc<SuiClient>,
    pub sender: SuiAddress,
    pub keystore: Arc<Keystore>,
}

impl SignedTransactionApi {
    pub async fn from_context(mut context: WalletContext) -> anyhow::Result<Self> {
        let client = context.get_client().await?;
        let sender = context.active_address()?;
        let keystore = context.config.into_inner().keystore;
        Ok(Self {
            client: Arc::new(client),
            sender,
            keystore: Arc::new(keystore),
        })
    }

    pub fn new(
        client: Arc<SuiClient>,
        sender: SuiAddress,
        keystore: Arc<Keystore>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            client,
            sender,
            keystore,
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
        let signature =
            self.keystore
                .sign_secure(&self.sender, &tx_data, Intent::sui_transaction())?;

        let transaction =
            Transaction::from_data(tx_data, Intent::sui_transaction(), vec![signature])
                .verify(&Default::default())?;
        let request_type = Some(ExecuteTransactionRequestType::WaitForLocalExecution);
        Ok(self
            .client
            .quorum_driver_api()
            .execute_transaction_block(transaction.into(), options, request_type)
            .await?)
    }

    pub async fn sign_and_execute_with_effects(
        &self,
        tx_data: TransactionData,
    ) -> anyhow::Result<SuiTransactionBlockResponse> {
        let options = SuiTransactionBlockResponseOptions::new().with_effects();
        self.sign_and_execute(tx_data, options).await
    }

    pub async fn get_coin_amount(
        &self,
        amount: u64,
        coin_type: String,
        gas: GasInfo,
    ) -> anyhow::Result<ObjectID> {
        let coins = get_all_coins(&self.client, self.sender, coin_type.clone()).await?;

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

        if let Some(i) = equal {
            return Ok(coins.data[i].coin_object_id);
        }

        if let Some(i) = greater {
            let primary = &coins.data[i];
            let GasInfo { object: gas_obj, budget } = gas;
            let tx_data = self
                .client
                .transaction_builder()
                .split_coin(
                    self.sender,
                    primary.coin_object_id,
                    vec![amount, primary.balance - amount],
                    gas_obj,
                    budget,
                )
                .await?;
            let options = SuiTransactionBlockResponseOptions::new().with_effects().with_object_changes();
            let response = self.sign_and_execute(tx_data, options).await?;
            assert!(
                response.confirmed_local_execution.is_some()
                    && response.confirmed_local_execution.unwrap()
            );

            for change in response.object_changes.unwrap() {
                if let sui_sdk::rpc_types::ObjectChange::Created { object_type, object_id, .. } = change {
                    if Coin::is_coin(&object_type) && object_type.type_params[0].to_string() == coin_type {
                        let coin: Coin = self.client.read_api().read_object(object_id).await?;
                        if coin.value() == amount {
                            return Ok(object_id)
                        }
                    }
                }
            }

            bail!("Failed to find coin from split result");
        };

        bail!("No Coin<{coin_type}> with balance >= {amount} found for address {}", self.sender)
    }
}
