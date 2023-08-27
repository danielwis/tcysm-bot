CREATE TABLE auths (
    user_id INTEGER,
    role TEXT NOT NULL,
    status TEXT NOT NULL,
    passphrase TEXT,
    auth_type TEXT,
    kth_id TEXT,
    authenticated_at TEXT,
    PRIMARY KEY(user_id, role)
)
