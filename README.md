# Bank transactions tool

Simple command-line tool for handling basic transactions

## Specifications:
- service is made async so that it can be used in async context: streaming, multi-threading, etc.
- transactions id's are global for all clients
- inconsistent transactions are stored is database so that their id cannot be reused later
- we don't process any transactions for locked clients
- code improvements that are overkill for test assignment but should be implemented in production environment are specified as comments in code under "improvement:" tag

## Run
`cargo run -- tests/assets/transactions.csv`

## Run unit tests
`cargo test --workspace`
