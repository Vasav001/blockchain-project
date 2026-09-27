CREATE TABLE blocks (
    block_index INTEGER NOT NULL PRIMARY KEY,
    timestamp INTEGER NOT NULL,
    data TEXT NOT NULL,
    previous_hash TEXT NOT NULL,
    hash TEXT NOT NULL UNIQUE
);
