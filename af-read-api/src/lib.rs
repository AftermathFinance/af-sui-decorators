use std::sync::Arc;

use anyhow::Context;
use jsonrpsee::core::async_trait;
use serde::Deserialize;
use sui_sdk::{
    apis::ReadApi,
    rpc_types::{Coin, Page, SuiData, SuiObjectDataOptions},
    SuiClient,
};
use sui_types::base_types::{ObjectID, SuiAddress};

#[async_trait]
pub trait ReadObject {
    async fn read_object<T: for<'a> Deserialize<'a>>(
        &self,
        object_id: ObjectID,
    ) -> anyhow::Result<T>;
}

#[async_trait]
impl ReadObject for ReadApi {
    async fn read_object<T: for<'a> Deserialize<'a>>(
        &self,
        object_id: ObjectID,
    ) -> anyhow::Result<T> {
        let raw_object = self
            .get_object_with_options(object_id, SuiObjectDataOptions::default().with_bcs())
            .await?;
        raw_object
            .into_object()?
            .bcs
            .unwrap()
            .try_as_move()
            .unwrap()
            .deserialize()
            .context("Failure deserializing object")
    }
}

pub async fn print_all_coins(
    client: &Arc<SuiClient>,
    sender: SuiAddress,
    coin_type: String,
) -> anyhow::Result<()> {
    let balance = client
        .coin_read_api()
        .get_balance(sender, Some(coin_type.clone()))
        .await?;
    println!("{:?}", balance);
    let coins = get_all_coins(client, sender, coin_type).await?;
    for coin in coins.data {
        println!("{:?}", coin);
    }
    Ok(())
}

pub async fn get_all_coins(
    client: &Arc<SuiClient>,
    sender: SuiAddress,
    coin_type: String,
) -> Result<Page<Coin, ObjectID>, anyhow::Error> {
    Ok(client
        .coin_read_api()
        .get_coins(sender, Some(coin_type), None, None)
        .await?)
}

pub async fn print_owned_objects(
    sui: &SuiClient,
    address: SuiAddress,
) -> Result<(), anyhow::Error> {
    let objects = sui
        .read_api()
        .get_owned_objects(address, None, None, None)
        .await?;
    for obj in objects.data {
        println!("{:?}", obj);
    }
    Ok(())
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
