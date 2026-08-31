ALTER TABLE remotes ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;

UPDATE remotes AS remote
SET priority = (
    SELECT COUNT(*)
    FROM remotes AS preceding
    WHERE COALESCE(preceding.name, preceding.address) COLLATE NOCASE
              < COALESCE(remote.name, remote.address) COLLATE NOCASE
       OR (
            COALESCE(preceding.name, preceding.address) COLLATE NOCASE
                = COALESCE(remote.name, remote.address) COLLATE NOCASE
            AND preceding.id < remote.id
       )
);
