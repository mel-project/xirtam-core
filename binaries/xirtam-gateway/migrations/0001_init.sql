CREATE TABLE IF NOT EXISTS gateway_meta (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    created_at INTEGER NOT NULL
);
