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

CREATE TABLE IF NOT EXISTS updates (
    height INTEGER NOT NULL,
    key_str TEXT NOT NULL,
    idx INTEGER NOT NULL,
    update_blob BLOB NOT NULL,
    PRIMARY KEY (height, key_str, idx)
);

CREATE INDEX IF NOT EXISTS updates_by_key ON updates (key_str, height, idx);
