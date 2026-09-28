-- The page a download was found on, when known (widget detections carry
-- their source page). Sent as the Referer/Origin header for direct HTTP
-- downloads (tube/KVS sites that reject a hotlinked GET without it).
ALTER TABLE downloads ADD COLUMN referer TEXT;
