pub mod logging;

use std::collections::HashMap;
use move_core_types::language_storage::StructTag;
use sui_sdk::rpc_types::{ObjectChange, SuiTransactionBlockResponse};
use sui_types::base_types::ObjectID;
use anyhow::anyhow;

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
