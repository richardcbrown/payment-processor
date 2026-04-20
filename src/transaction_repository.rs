use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;
use crate::transaction::Deposit;
use anyhow::Result;

#[derive(Clone)]
pub enum StoredTransaction {
    Deposit(Deposit),
}

impl StoredTransaction {
    pub fn get_transaction_id(&self) -> u32 {
        match self {
            Self::Deposit(deposit) => deposit.transaction_id,
        }
    }
}

#[async_trait]
pub trait TransactionRepository {
    async fn create_transaction(&self, transaction: StoredTransaction) -> Result<StoredTransaction>;

    async fn update_transaction(&self, transaction: StoredTransaction) -> Result<StoredTransaction>;

    async fn get_transaction_by_id(&self, transaction_id: u32) -> Result<Option<StoredTransaction>>;
}

pub struct InMemoryTransactionRepository {
    pub transactions: Arc<RwLock<HashMap<u32, StoredTransaction>>>,
}

impl InMemoryTransactionRepository {
    pub fn new() -> InMemoryTransactionRepository {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl TransactionRepository for InMemoryTransactionRepository {
    async fn create_transaction(&self, transaction: StoredTransaction) -> Result<StoredTransaction> {
        let mut write_store = self.transactions.write().await;

        write_store.insert(transaction.get_transaction_id(), transaction.clone());

        Ok(transaction)
    }

    async fn update_transaction(&self, transaction: StoredTransaction) -> Result<StoredTransaction> {
        let mut write_store = self.transactions.write().await;

        if write_store.contains_key(&transaction.get_transaction_id()) {
            write_store.insert(transaction.get_transaction_id(), transaction.clone());
        }

        Ok(transaction)
    }

    async fn get_transaction_by_id(&self, transaction_id: u32) -> Result<Option<StoredTransaction>> {
        let read_store = self.transactions.read().await;

        Ok(read_store.get(&transaction_id).cloned())
    }
}