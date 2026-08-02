CREATE TABLE profile (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    display_name TEXT NOT NULL
);

INSERT INTO profile (id, display_name) VALUES (1, '');
