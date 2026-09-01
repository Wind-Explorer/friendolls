CREATE TABLE app_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    onboarding_done INTEGER NOT NULL DEFAULT 0 CHECK (onboarding_done IN (0, 1))
);

INSERT INTO app_settings (id) VALUES (1);
