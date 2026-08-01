CREATE TABLE keypair (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    public_key TEXT NOT NULL,
    secret_key TEXT NOT NULL
);
