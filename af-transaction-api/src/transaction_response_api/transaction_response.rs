use anyhow::{anyhow, bail};
use sui_sdk::rpc_types::{
    ObjectChange, SuiExecutionStatus, SuiTransactionBlockEffects, SuiTransactionBlockResponse,
};
use sui_types::base_types::ObjectID;

pub struct TransactionResponse {
    package_id: Option<ObjectID>,
    object_changes: Option<Vec<ObjectChange>>,
    execution_status: Option<SuiExecutionStatus>,
}

impl TryFrom<SuiTransactionBlockResponse> for TransactionResponse {
    type Error = anyhow::Error;

    fn try_from(mut value: SuiTransactionBlockResponse) -> Result<Self, Self::Error> {
        let mut package = None;
        let object_changes = value.object_changes.take();

        if object_changes.is_some() {
            for change in object_changes.as_ref().unwrap() {
                if let ObjectChange::Published { package_id, .. } = change {
                    package = Some(*package_id);
                }
            }
        }

        let effects = if let Some(SuiTransactionBlockEffects::V1(effects)) = &value.effects {
            Some(effects.status.clone())
        } else {
            None
        };

        Ok(Self {
            package_id: package.map(|x| x.into()),
            object_changes,
            execution_status: effects,
        })
    }
}

impl TransactionResponse {
    pub fn check_execution_status(&self) -> anyhow::Result<()> {
        if self.execution_status.is_none() {
            return Ok(());
        }
        if let SuiExecutionStatus::Failure { error } = &self.execution_status.as_ref().unwrap() {
            bail!("Transaction failed with status:\n{error}");
        }
        Ok(())
    }

    pub fn object_changes(&self) -> anyhow::Result<&Vec<ObjectChange>> {
        self.object_changes
            .as_ref()
            .ok_or_else(|| anyhow!("No object changes in transaction"))
    }

    pub fn package_id(&self) -> anyhow::Result<&ObjectID> {
        self.package_id
            .as_ref()
            .ok_or_else(|| anyhow!("Missing package id in tx response"))
    }
}
