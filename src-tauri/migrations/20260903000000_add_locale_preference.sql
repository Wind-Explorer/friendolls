ALTER TABLE app_settings
ADD COLUMN locale_preference TEXT NOT NULL DEFAULT 'system';
