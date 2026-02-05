CREATE TABLE user_profiles (
    username TEXT NOT NULL PRIMARY KEY,
    profile BLOB NOT NULL,
    created INTEGER NOT NULL
);

CREATE INDEX user_profiles_username_idx
    ON user_profiles (username);
