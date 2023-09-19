CREATE TABLE authenticated (
    discord_id INTEGER PRIMARY KEY,
    kth_id TEXT NOT NULL,
    timestamp TEXT NOT NULL
);

CREATE TABLE pending_auths (
    discord_id INTEGER PRIMARY KEY,
    kth_id TEXT NOT NULL,
    verification_code TEXT NOT NULL
)
