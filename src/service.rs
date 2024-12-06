use crate::{
    client_id::ClientId,
    client_info::ClientInfo,
    input_file_reader::{InputFileRecord, InputFileRecordType},
    output_record::{OutputRecord, OutputRecordProvider},
    transaction_id::TransactionId,
    transaction_info::{TransactionInfo, TransactionStatus, TransactionType},
};
use anyhow::{bail, Context};
use log::{debug, error, warn};
use std::collections::{HashMap, HashSet};

pub struct Service {
    transaction_table: HashMap<TransactionId, TransactionInfo>,
    client_table: HashMap<ClientId, ClientInfo>,
    dispute_table: HashMap<TransactionId, ClientId>,
    chargeback_table: HashSet<TransactionId>,
}

impl Service {
    pub fn new() -> Service {
        Self {
            transaction_table: Default::default(),
            client_table: Default::default(),
            dispute_table: Default::default(),
            chargeback_table: Default::default(),
        }
    }

    fn process_deposit(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
        amount: Option<f64>,
    ) -> anyhow::Result<()> {
        if self.transaction_table.contains_key(&transaction_id) {
            bail!("duplicate transaction id: {transaction_id}");
        }
        let amount = amount.context("deposit transaction missing 'amount' field")?;
        self.transaction_table.insert(
            transaction_id,
            TransactionInfo {
                r#type: TransactionType::Deposit,
                client: client_id,
                amount: Some(amount),
                status: TransactionStatus::Success,
            },
        );
        if amount < 0f64 {
            let transaction_info = self
                .transaction_table
                .get_mut(&transaction_id)
                .expect("transaction id should exist");
            transaction_info.status = TransactionStatus::Failure;
            bail!("cannot deposit negative amount: {amount}");
        }
        let client_info = self.client_table.entry(client_id).or_default();
        // improvement: replace with checked_add/checked_sub
        client_info.available += amount;
        Ok(())
    }

    fn process_withdrawal(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
        amount: Option<f64>,
    ) -> anyhow::Result<()> {
        if self.transaction_table.contains_key(&transaction_id) {
            bail!("duplicate transaction id: {transaction_id}");
        }
        let amount = amount.context("withdrawal transaction missing 'amount' field")?;
        self.transaction_table.insert(
            transaction_id,
            TransactionInfo {
                r#type: TransactionType::Withdrawal,
                client: client_id,
                amount: Some(amount),
                status: TransactionStatus::Success,
            },
        );
        let transaction_info = self
            .transaction_table
            .get_mut(&transaction_id)
            .expect("transaction id should exist");
        if amount < 0f64 {
            transaction_info.status = TransactionStatus::Failure;
            bail!("cannot withdraw negative amount: {amount}");
        }
        let client_info = self
            .client_table
            .get_mut(&client_id)
            .context(format!("client id not found: {client_id}"))?;
        if client_info.available < amount {
            error!(
                "not enough funds for withdrawal, available: {}, requested: {amount}",
                client_info.available
            );
            transaction_info.status = TransactionStatus::Failure;
        } else {
            client_info.available -= amount;
        }
        Ok(())
    }

    fn process_dispute(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> anyhow::Result<()> {
        if !self.transaction_table.contains_key(&transaction_id) {
            bail!("dispute failure, transaction not found: {transaction_id}");
        }
        if !self.client_table.contains_key(&client_id) {
            bail!("dispute failure, client not found: {client_id}");
        }
        let transaction_info = self
            .transaction_table
            .get_mut(&transaction_id)
            .expect("transaction id should exist");
        Self::validate_dispute_transaction(client_id, transaction_info)?;
        let client_info = self
            .client_table
            .get_mut(&client_id)
            .context(format!("client id not found: {client_id}"))?;
        let amount = transaction_info
            .amount
            .context("transaction missing 'amount' field")?;
        if client_info.available < amount {
            bail!(
                "dispute failure, not enough funds, available: {}, requested: {amount}",
                client_info.available
            );
        }
        // improvement: execute operations atomically -> create AtomicTransaction class
        self.dispute_table.insert(transaction_id, client_id);
        client_info.available -= amount;
        client_info.on_hold += amount;
        Ok(())
    }

    fn validate_dispute_transaction(
        client_id: ClientId,
        transaction_info: &mut TransactionInfo,
    ) -> anyhow::Result<()> {
        if transaction_info.r#type != TransactionType::Deposit {
            bail!(
                "dispute failure, incorrect transaction type: {}",
                transaction_info.r#type
            );
        }
        if transaction_info.status != TransactionStatus::Success {
            bail!(
                "dispute failure, incorrect transaction state: {}",
                transaction_info.status
            );
        }
        if transaction_info.client != client_id {
            bail!(
                "dispute failure, incorrect transaction client id: {}",
                transaction_info.client
            );
        }
        Ok(())
    }

    fn process_resolve(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> anyhow::Result<()> {
        if !self.dispute_table.contains_key(&transaction_id) {
            bail!("resolve failure, transaction not found: {transaction_id}");
        }
        let dispute_client_id = self
            .dispute_table
            .get(&transaction_id)
            .expect("transaction id should exist");
        if dispute_client_id != &client_id {
            // improvement: remove sensitive information from logs
            bail!("resolve failure, client id mismatch: requested client id: {client_id}, existing client id: {dispute_client_id}");
        }
        let client_info = self
            .client_table
            .get_mut(&client_id)
            .context(format!("client id not found: {client_id}"))?;
        let transaction_info = self
            .transaction_table
            .get(&transaction_id)
            .expect("transaction id should exist");
        assert_ne!(transaction_info.status, TransactionStatus::Failure);
        let amount = transaction_info
            .amount
            .context("transaction missing 'amount' field")?;
        self.dispute_table.remove(&transaction_id);
        self.chargeback_table.insert(transaction_id);
        // improvement: resolve is a mirror operation to holding money and should be done in one place.
        // Possible solution: implement ReversableAction class where on "exec" you hold the money
        // and on "reverse" you do the opposite
        client_info.available += amount;
        assert!(client_info.on_hold >= amount);
        client_info.on_hold -= amount;
        Ok(())
    }

    fn process_chargeback(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> anyhow::Result<()> {
        if !self.chargeback_table.contains(&transaction_id) {
            bail!("chargeback failure, transaction not found: {transaction_id}");
        }
        let transaction_info = self
            .transaction_table
            .get(&transaction_id)
            .expect("transaction id should exist");
        if transaction_info.client != client_id {
            // improvement: remove sensitive information from logs
            bail!("chargeback failure, client id mismatch: requested client id: {client_id}, existing client id: {}", transaction_info.client);
        }

        let client_info = self
            .client_table
            .get_mut(&client_id)
            .context(format!("client id not found: {client_id}"))?;
        let amount = transaction_info
            .amount
            .context("transaction missing 'amount' field")?;
        if client_info.available < amount {
            bail!(
                "not enough funds for chargeback, available: {}, requested: {amount}",
                client_info.available
            );
        }

        self.chargeback_table.remove(&transaction_id);
        client_info.available -= amount;
        client_info.is_locked = true;
        Ok(())
    }
}

pub trait TransactionRecordHandler {
    async fn handle(&mut self, record: &InputFileRecord) -> anyhow::Result<()>;
}

impl TransactionRecordHandler for Service {
    async fn handle(&mut self, record: &InputFileRecord) -> anyhow::Result<()> {
        debug!("handle_transaction: {record:?}");
        let &InputFileRecord {
            r#type,
            client,
            tx,
            amount,
        } = record;

        let client_id = ClientId::new(client as u16);
        if self.client_table.contains_key(&client_id) {
            let client_info = self
                .client_table
                .get(&client_id)
                .context(format!("client id not found: {client_id}"))?;
            if r#type != InputFileRecordType::Resolve && client_info.is_locked {
                warn!("ignore transaction for locked client: {client_id}");
                return Ok(());
            }
        }

        let transaction_id = TransactionId::new(tx);

        match r#type {
            InputFileRecordType::Deposit => {
                self.process_deposit(transaction_id, client_id, amount)?;
            }
            InputFileRecordType::Withdrawal => {
                self.process_withdrawal(transaction_id, client_id, amount)?;
            }
            InputFileRecordType::Dispute => {
                self.process_dispute(transaction_id, client_id)?;
            }
            InputFileRecordType::Resolve => {
                self.process_resolve(transaction_id, client_id)?;
            }
            InputFileRecordType::Chargeback => {
                self.process_chargeback(transaction_id, client_id)?;
            }
        }
        Ok(())
    }
}

impl OutputRecordProvider for Service {
    fn get_records(&self) -> impl Iterator<Item = OutputRecord> {
        self.client_table
            .iter()
            .map(|(client_id, info)| OutputRecord {
                client: client_id.value() as u64,
                available: info.available,
                held: info.on_hold,
                total: info.on_hold + info.available,
                locked: info.is_locked,
            })
    }
}

// improvement: add more tests:
// - incorrect client id
// - incorrect transaction id
// - reading input file/writing output file
#[cfg(test)]
mod tests {
    use crate::{
        input_file_reader::{InputFileRecord, InputFileRecordType},
        output_record::OutputRecord,
        *,
    };
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup() -> Service {
        INIT.call_once(env_logger::init);
        Service::new()
    }

    #[tokio::test]
    async fn deposit_and_withdrawal() {
        let mut service = setup();
        service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Deposit,
                client: 1,
                tx: 0,
                amount: Some(10.0),
            })
            .await
            .expect("service failed to handle deposit request");
        let records = service.get_records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let OutputRecord {
            client,
            available,
            held,
            total,
            locked,
        } = records[0];
        assert_eq!(client, 1);
        assert_eq!(available, 10.0);
        assert_eq!(held, 0.0);
        assert_eq!(total, 10.0);
        assert!(!locked);

        service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Withdrawal,
                client: 1,
                tx: 1,
                amount: Some(1.5),
            })
            .await
            .expect("service failed to handle withdrawal request");

        let records = service.get_records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let OutputRecord {
            client,
            available,
            held,
            total,
            locked,
        } = records[0];
        assert_eq!(client, 1);
        assert_eq!(available, 8.5);
        assert_eq!(held, 0.0);
        assert_eq!(total, 8.5);
        assert!(!locked);
    }

    #[tokio::test]
    async fn chargeback() {
        let mut service = setup();
        service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Deposit,
                client: 1,
                tx: 0,
                amount: Some(10.0),
            })
            .await
            .expect("service failed to handle deposit request");
        let records = service.get_records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let OutputRecord {
            client,
            available,
            held,
            total,
            locked,
        } = records[0];
        assert_eq!(client, 1);
        assert_eq!(available, 10.0);
        assert_eq!(held, 0.0);
        assert_eq!(total, 10.0);
        assert!(!locked);

        service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Dispute,
                client: 1,
                tx: 0,
                amount: None,
            })
            .await
            .expect("service failed to handle dispute request");

        let records = service.get_records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let OutputRecord {
            client,
            available,
            held,
            total,
            locked,
        } = records[0];
        assert_eq!(client, 1);
        assert_eq!(available, 0.0);
        assert_eq!(held, 10.0);
        assert_eq!(total, 10.0);
        assert!(!locked);

        service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Resolve,
                client: 1,
                tx: 0,
                amount: None,
            })
            .await
            .expect("service failed to handle resolution request");

        let records = service.get_records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let OutputRecord {
            client,
            available,
            held,
            total,
            locked,
        } = records[0];
        assert_eq!(client, 1);
        assert_eq!(available, 10.0);
        assert_eq!(held, 0.0);
        assert_eq!(total, 10.0);
        assert!(!locked);

        service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Chargeback,
                client: 1,
                tx: 0,
                amount: None,
            })
            .await
            .expect("service failed to handle chargeback request");

        let records = service.get_records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let OutputRecord {
            client,
            available,
            held,
            total,
            locked,
        } = records[0];
        assert_eq!(client, 1);
        assert_eq!(available, 0.0);
        assert_eq!(held, 0.0);
        assert_eq!(total, 0.0);
        assert!(locked);
    }

    #[tokio::test]
    async fn overdraft() {
        let mut service = setup();
        service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Deposit,
                client: 2,
                tx: 3,
                amount: Some(10.0),
            })
            .await
            .expect("service failed to handle deposit request");
        let records = service.get_records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let OutputRecord {
            client,
            available,
            held,
            total,
            locked,
        } = records[0];
        assert_eq!(client, 2);
        assert_eq!(available, 10.0);
        assert_eq!(held, 0.0);
        assert_eq!(total, 10.0);
        assert!(!locked);

        service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Withdrawal,
                client: 2,
                tx: 0,
                amount: Some(11.0),
            })
            .await
            .expect("service failed to handle withdrawal request");

        let records = service.get_records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let OutputRecord {
            client,
            available,
            held,
            total,
            locked,
        } = records[0];
        assert_eq!(client, 2);
        assert_eq!(available, 10.0);
        assert_eq!(held, 0.0);
        assert_eq!(total, 10.0);
        assert!(!locked);
    }

    #[tokio::test]
    async fn negative_deposit() {
        let mut service = setup();
        let res = service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Deposit,
                client: 1,
                tx: 0,
                amount: Some(-10.0),
            })
            .await;
        assert!(res.is_err());
        assert_eq!(
            res.err().unwrap().to_string(),
            "cannot deposit negative amount: -10"
        );
    }

    #[tokio::test]
    async fn negative_withdrawal() {
        let mut service = setup();
        service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Deposit,
                client: 1,
                tx: 0,
                amount: Some(10.0),
            })
            .await
            .expect("service failed to handle deposit request");
        let records = service.get_records().collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        let OutputRecord {
            client,
            available,
            held,
            total,
            locked,
        } = records[0];
        assert_eq!(client, 1);
        assert_eq!(available, 10.0);
        assert_eq!(held, 0.0);
        assert_eq!(total, 10.0);
        assert!(!locked);

        let res = service
            .handle(&InputFileRecord {
                r#type: InputFileRecordType::Withdrawal,
                client: 1,
                tx: 1,
                amount: Some(-1.5),
            })
            .await;
        assert!(res.is_err());
        assert_eq!(
            res.err().unwrap().to_string(),
            "cannot withdraw negative amount: -1.5"
        );
    }
}
