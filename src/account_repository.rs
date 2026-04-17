use std::collections::HashMap;
use std::sync::{Arc};
use tokio::sync::RwLock;
use rust_decimal::Decimal;
use async_trait::async_trait;
use rust_decimal::prelude::Zero;
use anyhow::Result;

#[derive(Clone)]
pub struct Account {
    pub client_id: u16,
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
    pub total: Decimal,
}

impl Account {
    pub fn new (client_id: u16) -> Account {
        Self {
            client_id,
            available: Decimal::zero(),
            held: Decimal::zero(),
            total: Decimal::zero(),
            locked: false,
        }
    }

    pub fn deposit(&mut self, amount: Decimal) {
        self.available += amount;
        self.total += amount;
    }
}

#[async_trait]
pub trait AccountRepository {
    async fn get_account_by_client(&self, client_id: u16) -> Result<Account>;

    async fn set_account(&self, account: Account) -> Result<()>;
}

pub struct InMemoryAccountRepository {
   pub accounts: Arc<RwLock<HashMap<u16, Account>>>,
}
impl InMemoryAccountRepository {
    pub fn new() -> InMemoryAccountRepository {
        Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl AccountRepository for InMemoryAccountRepository {
    async fn get_account_by_client(&self, client_id: u16) ->  Result<Account> {
        {
            let read_store = self.accounts.read().await;

            let account = read_store.get(&client_id).cloned();

            drop(read_store);

            if let Some(account) = account {
                return Ok(account);
            }
        }

        let mut write_store = self.accounts.write().await;

        let new_account = Account::new(client_id);

        write_store.insert(client_id, new_account.clone());

        Ok(new_account)
    }

    async fn set_account(&self, account: Account) -> Result<()> {
        let mut write_store = self.accounts.write().await;

        write_store.insert(account.client_id, account);

        Ok(())
    }
}