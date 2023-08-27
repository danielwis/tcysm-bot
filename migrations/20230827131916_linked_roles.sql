-- Add migration script here
CREATE TABLE linked_roles (
    passphrase TEXT NOT NULL,
    role TEXT NOT NULL,
    PRIMARY KEY(passphrase, role)
)
