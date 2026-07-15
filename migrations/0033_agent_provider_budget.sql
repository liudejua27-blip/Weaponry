PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE agent_provider_daily_budgets (
  day_utc TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  budget_micros INTEGER NOT NULL CHECK (budget_micros > 0),
  spent_micros INTEGER NOT NULL DEFAULT 0 CHECK (spent_micros >= 0),
  reserved_micros INTEGER NOT NULL DEFAULT 0 CHECK (reserved_micros >= 0),
  unmetered_turns INTEGER NOT NULL DEFAULT 0 CHECK (unmetered_turns >= 0),
  updated_at TEXT NOT NULL,
  PRIMARY KEY(day_utc, provider_id)
);

INSERT INTO schema_migrations(version, name)
VALUES ('0033', 'agent_provider_budget');

COMMIT;
