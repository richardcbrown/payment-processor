use std::sync::Arc;
use crate::account_repository::AccountRepository;
use crate::transaction::{Deposit, Transaction};
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal::prelude::Zero;

pub struct PaymentProcessor {
    pub account_repository: Arc<dyn AccountRepository + Send + Sync>
}

impl PaymentProcessor {
    pub fn new(account_repository: Arc<dyn AccountRepository + Send + Sync>) -> PaymentProcessor {
        Self { account_repository }
    }

    pub async fn process_transaction(&self, transaction: Transaction) -> Result<()> {
        let client_id = transaction.get_client_id();

        let mut account = self.account_repository.get_account_by_client(client_id).await?;

        // if account is locked no further
        // transactions can be processed on it
        if account.locked {
            return Ok(());
        }

        match transaction {
            Transaction::Deposit(deposit) => {
                account.deposit(deposit.amount);

                self.account_repository.set_account(account).await?;

                Ok(())
            }
            Transaction::Withdrawal(withdraw) => {
                account.withdraw(withdraw.amount);

                // if there are insufficient funds after
                // withdrawal we can't commit the transaction
                if (account.available < Decimal::zero()) {
                    return Ok(());
                }

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
    use rust_decimal::prelude::Zero;
    use crate::account_repository::{Account, AccountRepository, InMemoryAccountRepository};
    use crate::payment_processor::PaymentProcessor;
    use crate::transaction::{Deposit, Transaction, Withdrawal};

    #[tokio::test]
    async fn it_processes_deposit() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());

        let processor = PaymentProcessor::new(account_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
    }

    #[tokio::test]
    async fn it_does_not_process_deposit_if_account_is_locked() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());

        let locked_account = Account {
            client_id: 1,
            available: Decimal::zero(),
            held: Decimal::zero(),
            total: Decimal::zero(),
            locked: true
        };

        account_repository.set_account(locked_account).await.unwrap();

        let processor = PaymentProcessor::new(account_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(0, 4));
        assert_eq!(account.available, Decimal::new(0, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
    }

    #[tokio::test]
    async fn it_processes_withdrawal() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());

        let processor = PaymentProcessor::new(account_repository.clone());

        let account = Account {
            client_id: 1,
            available: Decimal::new(10000, 4),
            held: Decimal::zero(),
            total: Decimal::new(10000, 4),
            locked: false
        };

        account_repository.set_account(account).await.unwrap();

        let withdrawal = Withdrawal::new(1, 1, Decimal::new(5000, 4));

        processor.process_transaction(Transaction::Withdrawal(withdrawal)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(5000, 4));
        assert_eq!(account.available, Decimal::new(5000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
    }

    #[tokio::test]
    async fn it_does_not_process_withdrawal_if_account_is_locked() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());

        let account = Account {
            client_id: 1,
            available: Decimal::new(10000, 4),
            held: Decimal::zero(),
            total: Decimal::new(10000, 4),
            locked: false
        };

        account_repository.set_account(account).await.unwrap();

        let processor = PaymentProcessor::new(account_repository.clone());

        let withdrawal = Withdrawal::new(1, 1, Decimal::new(5000, 4));

        processor.process_transaction(Transaction::Withdrawal(withdrawal)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
    }

    #[tokio::test]
    async fn it_does_not_process_withdrawal_if_account_has_insufficient_available_balance() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());

        let account = Account {
            client_id: 1,
            available: Decimal::new(10000, 4),
            held: Decimal::zero(),
            total: Decimal::new(10000, 4),
            locked: true
        };

        account_repository.set_account(account).await.unwrap();

        let processor = PaymentProcessor::new(account_repository.clone());

        let withdrawal = Withdrawal::new(1, 1, Decimal::new(20000, 4));

        processor.process_transaction(Transaction::Withdrawal(withdrawal)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
    }
}
