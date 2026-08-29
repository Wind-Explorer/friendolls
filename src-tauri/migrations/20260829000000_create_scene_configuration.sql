CREATE TABLE scene_configuration (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    puppet_scale REAL NOT NULL DEFAULT 1.0 CHECK (puppet_scale BETWEEN 0.5 AND 2.0)
);

INSERT INTO scene_configuration (id) VALUES (1);
