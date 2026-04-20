use std::{env, io};
use std::sync::Arc;
use crate::account_repository::{AccountRepository, InMemoryAccountRepository};
use crate::payment_processor::PaymentProcessor;
use crate::transaction::{RawTransaction, Transaction};
use crate::transaction_repository::{InMemoryTransactionRepository, TransactionRepository};

mod payment_processor;
mod account_repository;
mod transaction;
mod transaction_repository;
mod account;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let path = &args[1];

    let mut csv_reader = csv::Reader::from_path(path).unwrap();

    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Transaction>(1000);

    let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());
    let transaction_repository: Arc<dyn TransactionRepository + Send + Sync> = Arc::new(InMemoryTransactionRepository::new());

    let payment_processor = PaymentProcessor::new(account_repository.clone(), transaction_repository.clone());

    let processor_task = tokio::spawn(async move {
        while let Some(transaction) = receiver.recv().await {
            let result = payment_processor.process_transaction(transaction).await;

            if let Err(e) = result {
                eprintln!("Error while processing transaction: {}", e);
            }
        }
    });

    tokio::task::spawn(async move {
        for record in csv_reader.deserialize::<RawTransaction>() {
            let record = record
                .map_err(anyhow::Error::msg)
                .and_then(|r| r.to_transaction());

            match record {
                Ok(transaction) => {
                    let send_result = sender.send(transaction).await;

                    if let Err(e) = send_result {
                        eprintln!("Error while sending transaction: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Error while parsing transaction: {}", e);
                }
            }
        }
    }).await.unwrap();

    processor_task.await.unwrap();

    let accounts = account_repository.get_accounts().await.unwrap();

    let mut csv_writer = csv::Writer::from_writer(io::stdout());

    csv_writer.write_record(&["client", "available", "held", "total", "locked"]).unwrap();

    accounts.iter().for_each(|account| {
        csv_writer.serialize((
            account.client_id,
            account.available,
            account.held,
            account.total,
            account.locked
        )).unwrap();
    })
}
