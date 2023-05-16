use anyhow::{anyhow, bail};
use sui_sdk::rpc_types::{
    ObjectChange, SuiExecutionStatus, SuiTransactionBlockEffects, SuiTransactionBlockResponse,
};
use sui_types::base_types::ObjectID;

pub struct PublishedResponse {
    pub package_id: ObjectID,
    pub object_changes: Vec<ObjectChange>,
    pub execution_status: SuiExecutionStatus,
    pub response: SuiTransactionBlockResponse,
}

impl TryFrom<SuiTransactionBlockResponse> for PublishedResponse {
    type Error = anyhow::Error;

    fn try_from(mut value: SuiTransactionBlockResponse) -> Result<Self, Self::Error> {
        let mut package = None;
        let object_changes = value
            .object_changes
            .take()
            .ok_or_else(|| anyhow!("No object changes in transaction"))?;

        for change in &object_changes {
            if let ObjectChange::Published { package_id, .. } = change {
                package = Some(*package_id);
            }
        }

        let effects = if let Some(SuiTransactionBlockEffects::V1(effects)) = &value.effects {
            Ok(effects)
        } else {
            Err(anyhow::anyhow!(
                "No transaction effects in response {value:?}"
            ))
        }?;

        Ok(Self {
            package_id: package.ok_or_else(|| anyhow!("Missing package id in tx response"))?,
            object_changes,
            execution_status: effects.status.clone(),
            response: value,
        })
    }
}

impl PublishedResponse {
    pub fn check_execution_status(&self) -> anyhow::Result<()> {
        if let SuiExecutionStatus::Failure { error } = &self.execution_status {
            bail!("Transaction failed with status:\n{error}");
        }
        Ok(())
    }
}
