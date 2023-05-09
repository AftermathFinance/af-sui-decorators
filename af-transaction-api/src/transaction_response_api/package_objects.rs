use crate::transaction_response_api::transaction_response::TransactionResponse;
use move_core_types::language_storage::StructTag;
use std::collections::HashMap;
use sui_sdk::rpc_types::ObjectChange;
use sui_types::base_types::ObjectID;

pub struct CreatedObject {
    pub object_id: ObjectID,
    pub object_type: StructTag,
}

pub struct PackageObjects {
    pub package_id: ObjectID,
    pub objects: HashMap<String, Vec<CreatedObject>>,
}

impl TryFrom<TransactionResponse> for PackageObjects {
    type Error = anyhow::Error;

    fn try_from(value: TransactionResponse) -> Result<Self, Self::Error> {
        let mut objects = HashMap::<String, Vec<CreatedObject>>::new();
        for change in value.object_changes()? {
            if let ObjectChange::Created {
                object_type,
                object_id,
                ..
            } = change
            {
                let key = object_type.module.to_string() + "::" + object_type.name.as_str();
                let created = CreatedObject {
                    object_id: object_id.clone(),
                    object_type: object_type.clone(),
                };
                objects.entry(key).or_default().push(created);
            }
        }

        Ok(Self {
            package_id: value.package_id()?.clone(),
            objects,
        })
    }
}
