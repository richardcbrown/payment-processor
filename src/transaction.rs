use rust_decimal::Decimal;
use serde::Deserialize;
use anyhow::{anyhow, Result};

pub struct Deposit {
    pub client_id: u16,
    pub transaction_id: u32,
    pub amount: Decimal
}

impl Deposit {
    pub fn new(client_id: u16, transaction_id: u32, amount: Decimal) -> Deposit {
        Self {
            client_id,
            transaction_id,
            amount
        }
    }
}

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

pub enum Transaction {
    Deposit(Deposit),
    Withdrawal(Withdrawal),
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
            _ => Err(anyhow!("Invalid transaction {}", self.r#type))
        }
    }
}

impl Transaction {
    pub fn get_client_id(&self) -> u16 {
        match self {
            Self::Deposit(deposit) => deposit.client_id,
            Self::Withdrawal(withdraw) => withdraw.client_id,
        }
    }
}