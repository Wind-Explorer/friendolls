ALTER TABLE scene_configuration
ADD COLUMN puppet_movement_mode TEXT NOT NULL DEFAULT 'free'
CHECK (puppet_movement_mode IN ('free', 'bottom'));
