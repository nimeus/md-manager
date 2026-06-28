-- Per-chunk content hash, so re-indexing a document can preserve unchanged chunks (and
-- their embeddings) instead of deleting + re-inserting everything (which would re-embed
-- the whole doc on every edit). The worker also uses it to skip re-embedding identical
-- content (dedup).

ALTER TABLE doc_chunks ADD COLUMN content_hash text NOT NULL DEFAULT '';
CREATE INDEX doc_chunks_content_hash_idx ON doc_chunks (content_hash);
