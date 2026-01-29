CREATE TABLE attachment_roots (
    hash BLOB PRIMARY KEY,
    root BLOB NOT NULL,
    sender_username TEXT NOT NULL
);

