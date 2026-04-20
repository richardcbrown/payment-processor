# Payment Processor

Usage:

```
cargo build

cargo run -- transaction.csv > accounts.csv
```

## Usage of AI

Claude code was used to perform a code review on the initial commit.
It noted that the original usage of a read + write lock in the account repository get_account_by_client
be changed to a single write_store.entry to prevent another task mutating the store between the read + write lock.