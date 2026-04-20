use std::sync::Arc;
use crate::account_repository::AccountRepository;
use crate::transaction::{Transaction, TransactionState};
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use rust_decimal::prelude::Zero;
use crate::transaction_repository::{StoredTransaction, TransactionRepository};

pub struct PaymentProcessor {
    pub account_repository: Arc<dyn AccountRepository + Send + Sync>,
    pub transaction_repository: Arc<dyn TransactionRepository + Send + Sync>
}

impl PaymentProcessor {
    pub fn new(
        account_repository: Arc<dyn AccountRepository + Send + Sync>,
        transaction_repository: Arc<dyn TransactionRepository + Send + Sync>
    ) -> PaymentProcessor {
        Self { account_repository, transaction_repository }
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

                self.transaction_repository.create_transaction(StoredTransaction::Deposit(deposit)).await?;

                self.account_repository.set_account(account).await?;

                Ok(())
            }
            Transaction::Withdrawal(withdraw) => {
                account.withdraw(withdraw.amount);

                // if there are insufficient funds after
                // withdrawal we can't commit the transaction
                if account.available < Decimal::zero() {
                    return Ok(());
                }

                self.account_repository.set_account(account).await?;

                Ok(())
            }
            Transaction::Dispute(dispute) => {
                // get the disputed transaction
                let disputed_transaction = self.transaction_repository.get_transaction_by_id(dispute.transaction_id).await?;

                if let Some(disputed_transaction) = disputed_transaction {
                    match disputed_transaction {
                        // only deposit transactions can be disputed
                        StoredTransaction::Deposit(mut deposit) => {
                            if deposit.client_id != dispute.client_id {
                                return Err(anyhow!("Deposit client id {} does not match transaction client id {}", deposit.client_id, dispute.transaction_id));
                            }

                            account.dispute(deposit.amount);

                            deposit.state = Some(TransactionState::Disputed);

                            self.account_repository.set_account(account).await?;
                            self.transaction_repository.update_transaction(StoredTransaction::Deposit(deposit)).await?;
                        }
                    }
                }

                Ok(())
            }
            Transaction::Resolve(resolve) => {
                // find disputed transaction
                let disputed_transaction = self.transaction_repository.get_transaction_by_id(resolve.transaction_id).await?;

                if let Some(disputed_transaction) = disputed_transaction {
                    match disputed_transaction {
                        StoredTransaction::Deposit(mut deposit) => {
                            // invalid transaction doesn't match the original client id
                            if deposit.client_id != resolve.client_id {
                                return Err(anyhow!("Deposit client id {} does not match transaction client id {}", deposit.client_id, resolve.transaction_id));
                            }

                            // check transaction is under dispute
                            if let Some(state) = deposit.state {
                                match state {
                                    TransactionState::Disputed => {
                                        account.resolve(deposit.amount);

                                        deposit.state = Some(TransactionState::Resolved);

                                        self.account_repository.set_account(account).await?;
                                        self.transaction_repository.update_transaction(StoredTransaction::Deposit(deposit)).await?;
                                    }
                                    // not under dispute, so do nothing
                                    _ => return Ok(())
                                }
                            }
                        }
                    }
                }

                Ok(())
            }
            Transaction::Chargeback(chargeback) => {
                let disputed_transaction = self.transaction_repository.get_transaction_by_id(chargeback.transaction_id).await?;

                if let Some(disputed_transaction) = disputed_transaction {
                    match disputed_transaction {
                        StoredTransaction::Deposit(mut deposit) => {
                            // invalid transaction doesn't match the original client id
                            if deposit.client_id != chargeback.client_id {
                                return Err(anyhow!("Deposit client id {} does not match transaction client id {}", deposit.client_id, chargeback.transaction_id));
                            }

                            // check transaction is under dispute
                            if let Some(state) = deposit.state {
                                match state {
                                    TransactionState::Disputed => {
                                        account.chargeback(deposit.amount);

                                        deposit.state = Some(TransactionState::ChargedBack);

                                        self.account_repository.set_account(account).await?;
                                        self.transaction_repository.update_transaction(StoredTransaction::Deposit(deposit)).await?;
                                    }
                                    // not under dispute, so do nothing
                                    _ => return Ok(())
                                }
                            }
                        }
                    }
                }

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
    use crate::account::Account;
    use crate::account_repository::{AccountRepository, InMemoryAccountRepository};
    use crate::payment_processor::PaymentProcessor;
    use crate::transaction::{Chargeback, Deposit, Dispute, Resolve, Transaction, Withdrawal};
    use crate::transaction_repository::{InMemoryTransactionRepository, TransactionRepository};

    #[tokio::test]
    async fn it_processes_deposit() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());

        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

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
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());

        let locked_account = Account {
            client_id: 1,
            available: Decimal::zero(),
            held: Decimal::zero(),
            total: Decimal::zero(),
            locked: true
        };

        account_repository.set_account(locked_account).await.unwrap();

        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

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
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());

        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

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
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());

        let account = Account {
            client_id: 1,
            available: Decimal::new(10000, 4),
            held: Decimal::zero(),
            total: Decimal::new(10000, 4),
            locked: true
        };

        account_repository.set_account(account).await.unwrap();

        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

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
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());

        let account = Account {
            client_id: 1,
            available: Decimal::new(10000, 4),
            held: Decimal::zero(),
            total: Decimal::new(10000, 4),
            locked: true
        };

        account_repository.set_account(account).await.unwrap();

        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let withdrawal = Withdrawal::new(1, 1, Decimal::new(20000, 4));

        processor.process_transaction(Transaction::Withdrawal(withdrawal)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
    }

    #[tokio::test]
    async fn it_processes_dispute() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());

        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));

        let dispute = Dispute::new(deposit.client_id, deposit.transaction_id);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        let mut account = account_repository.get_account_by_client(1).await.unwrap();

        account.locked = true;

        account_repository.set_account(account).await.unwrap();

        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
        assert_eq!(account.locked, true);
    }

    #[tokio::test]
    async fn it_does_not_process_dispute_for_non_existent_transaction() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(deposit.client_id, 2);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();
        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
        assert_eq!(account.locked, false);
    }

    #[tokio::test]
    async fn it_does_not_process_dispute_for_locked_account() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(deposit.client_id, 2);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        let mut account = account_repository.get_account_by_client(1).await.unwrap();

        account.locked = true;

        account_repository.set_account(account).await.unwrap();

        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
        assert_eq!(account.locked, true);
    }

    #[tokio::test]
    async fn it_returns_error_if_dispute_clientid_does_not_match_original_transaction() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(2, 1);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        let result = processor.process_transaction(Transaction::Dispute(dispute)).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_processes_resolve() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(deposit.client_id, deposit.transaction_id);
        let resolve = Resolve::new(deposit.client_id, deposit.transaction_id);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
        assert_eq!(account.locked, false);

        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(0, 4));
        assert_eq!(account.held, Decimal::new(10000, 4));
        assert_eq!(account.locked, false);

        processor.process_transaction(Transaction::Resolve(resolve)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
        assert_eq!(account.locked, false);
    }

    #[tokio::test]
    async fn it_does_not_process_resolve_for_non_existent_transaction() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(deposit.client_id, 1);
        let resolve = Resolve::new(deposit.client_id, 2);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();
        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();
        processor.process_transaction(Transaction::Resolve(resolve)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(0, 4));
        assert_eq!(account.held, Decimal::new(10000, 4));
        assert_eq!(account.locked, false);
    }

    #[tokio::test]
    async fn it_does_not_process_resolve_for_locked_account() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(deposit.client_id, 1);
        let resolve = Resolve::new(deposit.client_id, 1);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();
        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();

        let mut account = account_repository.get_account_by_client(1).await.unwrap();

        account.locked = true;

        account_repository.set_account(account).await.unwrap();

        processor.process_transaction(Transaction::Resolve(resolve)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(0, 4));
        assert_eq!(account.held, Decimal::new(10000, 4));
        assert_eq!(account.locked, true);
    }

    #[tokio::test]
    async fn it_returns_error_if_resolve_clientid_does_not_match_original_transaction() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(2, 1);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        let result = processor.process_transaction(Transaction::Dispute(dispute)).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn it_processes_chargeback() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(deposit.client_id, deposit.transaction_id);
        let chargeback = Chargeback::new(deposit.client_id, deposit.transaction_id);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(10000, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
        assert_eq!(account.locked, false);

        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(0, 4));
        assert_eq!(account.held, Decimal::new(10000, 4));
        assert_eq!(account.locked, false);

        processor.process_transaction(Transaction::Chargeback(chargeback)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(0, 4));
        assert_eq!(account.available, Decimal::new(0, 4));
        assert_eq!(account.held, Decimal::new(0, 4));
        assert_eq!(account.locked, true);
    }

    #[tokio::test]
    async fn it_does_not_process_chargeback_for_non_existent_transaction() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(deposit.client_id, 1);
        let chargeback = Chargeback::new(deposit.client_id, 2);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();
        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();
        processor.process_transaction(Transaction::Chargeback(chargeback)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(0, 4));
        assert_eq!(account.held, Decimal::new(10000, 4));
        assert_eq!(account.locked, false);
    }

    #[tokio::test]
    async fn it_does_not_process_chargeback_for_locked_account() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(deposit.client_id, 1);
        let chargeback = Chargeback::new(deposit.client_id, 1);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();
        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();

        let mut account = account_repository.get_account_by_client(1).await.unwrap();

        account.locked = true;

        account_repository.set_account(account).await.unwrap();

        processor.process_transaction(Transaction::Chargeback(chargeback)).await.unwrap();

        let account = account_repository.get_account_by_client(1).await.unwrap();

        assert_eq!(account.client_id, 1);
        assert_eq!(account.total, Decimal::new(10000, 4));
        assert_eq!(account.available, Decimal::new(0, 4));
        assert_eq!(account.held, Decimal::new(10000, 4));
        assert_eq!(account.locked, true);
    }

    #[tokio::test]
    async fn it_returns_error_if_chargeback_clientid_does_not_match_original_transaction() {
        let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
        let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());
        let processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

        let deposit = Deposit::new(1, 1, Decimal::new(10000, 4));
        let dispute = Dispute::new(deposit.client_id, 1);
        let chargeback = Chargeback::new(2, 1);

        processor.process_transaction(Transaction::Deposit(deposit)).await.unwrap();

        processor.process_transaction(Transaction::Dispute(dispute)).await.unwrap();

        let result = processor.process_transaction(Transaction::Chargeback(chargeback)).await;

        assert!(result.is_err());
    }
}
