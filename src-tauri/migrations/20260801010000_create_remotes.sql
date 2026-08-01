CREATE TABLE remotes (
    id TEXT PRIMARY KEY NOT NULL,
    address TEXT NOT NULL,
    name TEXT,
    port INTEGER CHECK (port IS NULL OR port BETWEEN 0 AND 65535)
);
