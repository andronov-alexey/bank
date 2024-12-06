use anyhow::Context;
use csv::{Reader, ReaderBuilder, Trim};
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::{cmp::PartialEq, fs::File};

#[derive(Debug, Display, Deserialize, Copy, Clone, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InputFileRecordType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InputFileRecord {
    pub r#type: InputFileRecordType,
    pub client: u64,
    pub tx: u64,
    pub amount: Option<f64>,
}

pub struct InputFileReader {
    path: String,
}

impl InputFileReader {
    pub fn new(path: String) -> InputFileReader {
        Self { path }
    }

    pub fn read_file(&self) -> anyhow::Result<impl Iterator<Item = InputFileRecord>> {
        let mut rdr: Reader<File> = ReaderBuilder::new()
            .flexible(true)
            .trim(Trim::All)
            .from_path(self.path.clone())
            .context(format!("failed to open file: {}", self.path))?;
        Ok(rdr
            .deserialize()
            .filter_map(|record| record.ok())
            .collect::<Vec<InputFileRecord>>()
            .into_iter())
    }
}
