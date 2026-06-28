-- Embedding-worker bookkeeping: per-chunk backoff + dead-letter.
--
-- Without this, a chunk that always fails to embed (e.g. a provider rejects its
-- content) stays `embedding IS NULL` and is re-fetched at the head of the queue
-- every interval, starving every chunk behind it. These columns let the worker
-- push a failing chunk into the future (exponential backoff) and, after enough
-- consecutive failures, mark it dead so it is skipped entirely and surfaced to ops.
--
-- The `embedding` column itself is created at runtime by the worker (its width is
-- the env-driven dimension), so it is not referenced here; these columns are
-- dimension-independent and safe to add statically.

ALTER TABLE doc_chunks
    ADD COLUMN IF NOT EXISTS embed_attempts        integer     NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS embed_next_attempt_at timestamptz,
    ADD COLUMN IF NOT EXISTS embed_failed          boolean     NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS embed_last_error      text;
