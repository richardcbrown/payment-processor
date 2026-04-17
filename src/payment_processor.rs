use std::sync::Arc;
use crate::account_repository::AccountRepository;
use crate::transaction::{Deposit, Transaction};
use anyhow::Result;

pub struct PaymentProcessor {
    pub account_repository: Arc<dyn AccountRepository>
}

impl PaymentProcessor {
    pub fn new(account_repository: Arc<dyn AccountRepository>) -> PaymentProcessor {
        Self { account_repository }
    }

    pub async fn process_transaction(&self, transaction: Transaction) -> Result<()> {
        match transaction {
            Transaction::Deposit(deposit) => {
                let mut account = self.account_repository.get_account_by_client(deposit.client_id).await?;

                account.deposit(deposit.amount);

                self.account_repository.set_account(account).await?;

                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;
    use rust_decimal::Decimal;
    use crate::account_repository::{Account, AccountRepository, InMemoryAccountRepository};
    use crate::payment_processor::PaymentProcessor;
    use crate::transaction::{Deposit, Transaction};

    #[tokio::test]
    async fn it_processes_deposit() {
        let account_repository: Arc<dyn AccountRepository> = Arc::new(InMemoryAccountRepository::new());

        let processor = PaymentProcessor::new(account_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
    }
}
