CREATE TABLE device_temp_pks (
    device_hash BLOB NOT NULL PRIMARY KEY,
    temp_pk BLOB NOT NULL
);

CREATE UNIQUE INDEX device_auth_tokens_auth_token_uidx
    ON device_auth_tokens (auth_token);
