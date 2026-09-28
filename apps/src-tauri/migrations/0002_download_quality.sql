-- Per-download quality override (chosen at download time in the widget),
-- distinct from the global default in Settings. NULL max_height = "best
-- available"; audio_only 0/1. Applied by the RegistryPlanner when building
-- yt-dlp's format selector.
ALTER TABLE downloads ADD COLUMN max_height INTEGER;
ALTER TABLE downloads ADD COLUMN audio_only INTEGER NOT NULL DEFAULT 0;
