CREATE TABLE IF NOT EXISTS dir_headers (
    height INTEGER PRIMARY KEY,
    header BLOB NOT NULL,
    header_hash BLOB NOT NULL
);
