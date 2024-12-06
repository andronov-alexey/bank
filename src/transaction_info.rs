use crate::client_id::ClientId;
use derive_more::Display;
use serde::{Deserialize, Serialize};

#[derive(Debug, Display, Deserialize, Copy, Clone, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Display, Copy, Clone, Eq, PartialEq)]
pub enum TransactionStatus {
    Success,
    Failure,
}

#[derive(Debug, Copy, Clone)]
pub struct TransactionInfo {
    pub r#type: TransactionType,
    pub client: ClientId,
    pub amount: Option<f64>,
    pub status: TransactionStatus,
}
