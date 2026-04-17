use rust_decimal::Decimal;

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

pub enum Transaction {
    Deposit(Deposit),
}