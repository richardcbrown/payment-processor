use std::{env, io};
use std::sync::Arc;
use crate::account_repository::{AccountRepository, InMemoryAccountRepository};
use crate::payment_processor::PaymentProcessor;
use crate::transaction::{RawTransaction, Transaction};

mod payment_processor;
mod account_repository;
mod transaction;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let path = &args[1];

    let mut csv_reader = csv::Reader::from_path(path).unwrap();

    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Transaction>(1000);

    let account_repository: Arc<dyn AccountRepository + Send + Sync> = Arc::new(InMemoryAccountRepository::new());

    let payment_processor = PaymentProcessor::new(account_repository.clone());

    let processor_task = tokio::spawn(async move {
        while let Some(transaction) = receiver.recv().await {
            payment_processor.process_transaction(transaction).await.unwrap();
        }
    });

    tokio::task::spawn(async move {
        for record in csv_reader.deserialize() {
            let record: RawTransaction = record.unwrap();

            sender.send(record.to_transaction().unwrap()).await.unwrap();
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
