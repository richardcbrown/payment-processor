use rust_decimal::Decimal;
use serde::Deserialize;
use anyhow::{anyhow, Result};

#[derive(Clone)]
pub enum TransactionState {
    Disputed,
    Resolved,
    ChargedBack
}

#[derive(Clone)]
pub struct Deposit {
    pub client_id: u16,
    pub transaction_id: u32,
    pub amount: Decimal,
    pub state: Option<TransactionState>,
}

impl Deposit {
    pub fn new(client_id: u16, transaction_id: u32, amount: Decimal) -> Deposit {
        Self {
            client_id,
            transaction_id,
            amount,
            state: None,
        }
    }
}

#[derive(Clone)]
pub struct Withdrawal {
    pub client_id: u16,
    pub transaction_id: u32,
    pub amount: Decimal
}

impl Withdrawal {
    pub fn new(client_id: u16, transaction_id: u32, amount: Decimal) -> Withdrawal {
        Self {
            client_id,
            transaction_id,
            amount
        }
    }
}

#[derive(Clone)]
pub struct Dispute {
    pub client_id: u16,
    pub transaction_id: u32,
}

impl Dispute {
    pub fn new(client_id: u16, transaction_id: u32) -> Dispute {
        Self {
            client_id,
            transaction_id,
        }
    }
}

#[derive(Clone)]
pub struct Resolve {
    pub client_id: u16,
    pub transaction_id: u32,
}

impl Resolve {
    pub fn new(client_id: u16, transaction_id: u32) -> Resolve {
        Self {
            client_id,
            transaction_id,
        }
    }
}

#[derive(Clone)]
pub struct Chargeback {
    pub client_id: u16,
    pub transaction_id: u32,
}

impl Chargeback {
    pub fn new(client_id: u16, transaction_id: u32) -> Chargeback {
        Self {
            client_id,
            transaction_id,
        }
    }
}

#[derive(Clone)]
pub enum Transaction {
    Deposit(Deposit),
    Withdrawal(Withdrawal),
    Dispute(Dispute),
    Resolve(Resolve),
    Chargeback(Chargeback),
}

#[derive(Deserialize)]
pub struct RawTransaction {
    pub r#type: String,
    pub amount: Option<Decimal>,
    pub client: u16,
    pub tx: u32,
}

impl RawTransaction {
    pub fn to_transaction(&self) -> Result<Transaction> {
        match self.r#type.to_lowercase().as_str() {
            "deposit" => {
                if let Some(amount) = self.amount {
                   Ok(Transaction::Deposit(Deposit::new(self.client, self.tx, amount)))
                } else {
                    Err(anyhow!("Amount must be provided"))
                }
            }
            "withdrawal" => {
                if let Some(amount) = self.amount {
                    Ok(Transaction::Withdrawal(Withdrawal::new(self.client, self.tx, amount)))
                } else {
                    Err(anyhow!("Amount must be provided"))
                }
            }
            "dispute" => {
                Ok(Transaction::Dispute(Dispute::new(self.client, self.tx)))
            }
            "resolve" => {
                Ok(Transaction::Resolve(Resolve::new(self.client, self.tx)))
            }
            "chargeback" => {
                Ok(Transaction::Chargeback(Chargeback::new(self.client, self.tx)))
            }
            _ => Err(anyhow!("Invalid transaction {}", self.r#type))
        }
    }
}

impl Transaction {
    pub fn get_client_id(&self) -> u16 {
        match self {
            Self::Deposit(deposit) => deposit.client_id,
            Self::Withdrawal(withdraw) => withdraw.client_id,
            Self::Dispute(dispute) => dispute.client_id,
            Self::Resolve(resolve) => resolve.client_id,
            Self::Chargeback(chargeback) => chargeback.client_id,
        }
    }
}