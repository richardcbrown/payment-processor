use rust_decimal::Decimal;
use rust_decimal::prelude::Zero;
use serde::Serialize;

#[derive(Clone, Serialize)]
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

    pub fn withdraw(&mut self, amount: Decimal) {
        self.available -= amount;
        self.total -= amount;
    }

    pub fn dispute(&mut self, amount: Decimal) {
        self.available -= amount;
        self.held += amount;
    }

    pub fn resolve(&mut self, amount: Decimal) {
        self.available += amount;
        self.held -= amount;
    }

    pub fn chargeback(&mut self, amount: Decimal) {
        self.held -= amount;
        self.total -= amount;
        self.locked = true;
    }
}