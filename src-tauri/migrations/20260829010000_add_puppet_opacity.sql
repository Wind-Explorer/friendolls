ALTER TABLE scene_configuration
ADD COLUMN puppet_opacity REAL NOT NULL DEFAULT 1.0
CHECK (puppet_opacity BETWEEN 0.1 AND 1.0);
