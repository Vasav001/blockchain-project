CREATE TABLE block_transactions (
    block_index INTEGER NOT NULL REFERENCES blocks (block_index),
    transaction_index INTEGER NOT NULL,
    transaction_id TEXT NOT NULL,
    sender TEXT NOT NULL,
    recipient TEXT NOT NULL,
    amount INTEGER NOT NULL,
    PRIMARY KEY (block_index, transaction_index)
);
