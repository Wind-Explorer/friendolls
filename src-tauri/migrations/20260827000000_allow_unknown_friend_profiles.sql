CREATE TABLE friends_with_optional_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT
);

-- Existing names were entered locally and cannot be distinguished from names
-- learned from a signed remote profile. Keep the relationship, then learn the
-- authoritative display name again from the remote.
INSERT INTO friends_with_optional_profiles (id)
SELECT id FROM friends;

DROP TABLE friends;
ALTER TABLE friends_with_optional_profiles RENAME TO friends;
