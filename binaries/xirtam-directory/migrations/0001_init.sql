CREATE TABLE IF NOT EXISTS pow_seeds (
    seed BLOB PRIMARY KEY,
    use_before INTEGER NOT NULL,
    effort INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS headers (
    height INTEGER PRIMARY KEY,
    header BLOB NOT NULL,
    header_hash BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS chunks (
    height INTEGER PRIMARY KEY,
    chunk_blob BLOB NOT NULL
);
