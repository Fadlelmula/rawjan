-- ---------------------------------------------------------------------------
-- 0002 — integrity constraints and query indexes
--
-- 1. Money-component CHECK constraints on liability_payments and
--    investment_trades (added via table rebuild: SQLite cannot add CHECKs
--    with ALTER TABLE).
-- 2. A uniqueness guarantee for daily investment valuations (after removing
--    any duplicates that already exist, keeping the latest row).
-- 3. Indexes for foreign keys and tenant-scoped lookups that previously
--    required full table scans.
-- ---------------------------------------------------------------------------

-- 1a. liability_payments: components must be non-negative.
ALTER TABLE liability_payments RENAME TO liability_payments_old;

CREATE TABLE liability_payments (
  id             TEXT PRIMARY KEY,
  tenant_id      TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  liability_id   TEXT NOT NULL REFERENCES liabilities(id) ON DELETE CASCADE,
  paid_on        TEXT NOT NULL,
  amount_minor   INTEGER NOT NULL CHECK (amount_minor > 0),
  principal_minor INTEGER NOT NULL DEFAULT 0 CHECK (principal_minor >= 0),
  interest_minor INTEGER NOT NULL DEFAULT 0 CHECK (interest_minor >= 0),
  fee_minor      INTEGER NOT NULL DEFAULT 0 CHECK (fee_minor >= 0),
  transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
  note           TEXT,
  created_at     TEXT NOT NULL
) STRICT;

INSERT INTO liability_payments
  (id, tenant_id, liability_id, paid_on, amount_minor, principal_minor,
   interest_minor, fee_minor, transaction_id, note, created_at)
SELECT id, tenant_id, liability_id, paid_on, amount_minor, principal_minor,
       interest_minor, fee_minor, transaction_id, note, created_at
  FROM liability_payments_old;

DROP TABLE liability_payments_old;
CREATE INDEX idx_liability_payments ON liability_payments(liability_id, paid_on);

-- 1b. investment_trades: fees are non-negative, scales bounded.
ALTER TABLE investment_trades RENAME TO investment_trades_old;

CREATE TABLE investment_trades (
  id            TEXT PRIMARY KEY,
  tenant_id     TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  account_id    TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  instrument_id TEXT NOT NULL REFERENCES instruments(id) ON DELETE RESTRICT,
  side          TEXT NOT NULL CHECK (side IN ('buy','sell')),
  trade_date    TEXT NOT NULL,
  quantity_scaled INTEGER NOT NULL CHECK (quantity_scaled > 0),
  price_scaled  INTEGER NOT NULL CHECK (price_scaled >= 0),
  price_scale   INTEGER NOT NULL DEFAULT 8 CHECK (price_scale BETWEEN 0 AND 28),
  fee_minor     INTEGER NOT NULL DEFAULT 0 CHECK (fee_minor >= 0),
  currency_code TEXT NOT NULL REFERENCES currencies(code),
  fx_rate_id    TEXT REFERENCES exchange_rates(id) ON DELETE RESTRICT,
  transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
  notes         TEXT,
  created_at    TEXT NOT NULL
) STRICT;

INSERT INTO investment_trades
  (id, tenant_id, user_id, account_id, instrument_id, side, trade_date,
   quantity_scaled, price_scaled, price_scale, fee_minor, currency_code,
   fx_rate_id, transaction_id, notes, created_at)
SELECT id, tenant_id, user_id, account_id, instrument_id, side, trade_date,
       quantity_scaled, price_scaled, price_scale, fee_minor, currency_code,
       fx_rate_id, transaction_id, notes, created_at
  FROM investment_trades_old;

DROP TABLE investment_trades_old;
CREATE INDEX idx_trades_account ON investment_trades(tenant_id, account_id, trade_date);
CREATE INDEX idx_trades_instrument ON investment_trades(tenant_id, instrument_id, trade_date);

-- 2. One valuation per account, instrument and day: keep the most recent row
--    for any duplicates already recorded, then forbid new ones.
DELETE FROM investment_valuations
 WHERE id NOT IN (
   SELECT id FROM (
     SELECT id,
            ROW_NUMBER() OVER (
              PARTITION BY tenant_id, account_id, IFNULL(instrument_id, ''), as_of_date
              ORDER BY created_at DESC, id DESC
            ) AS rn
       FROM investment_valuations
   ) WHERE rn = 1
 );

CREATE UNIQUE INDEX idx_valuations_unique
  ON investment_valuations(tenant_id, account_id, IFNULL(instrument_id, ''), as_of_date);

-- 3. Foreign-key and tenant-scoped lookup indexes.
CREATE INDEX IF NOT EXISTS idx_tx_income_source ON transactions(tenant_id, income_source_id);
CREATE INDEX IF NOT EXISTS idx_tx_counter ON transactions(counter_account_id);
CREATE INDEX IF NOT EXISTS idx_tx_fx ON transactions(fx_rate_id);
CREATE INDEX IF NOT EXISTS idx_ledger_fx ON ledger_entries(fx_rate_id);
CREATE INDEX IF NOT EXISTS idx_splits_category ON transaction_splits(tenant_id, category_id);
CREATE INDEX IF NOT EXISTS idx_tags_tag ON transaction_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_liabilities_account ON liabilities(tenant_id, account_id);
CREATE INDEX IF NOT EXISTS idx_liabilities_tenant ON liabilities(tenant_id);
CREATE INDEX IF NOT EXISTS idx_valuations_instrument ON investment_valuations(tenant_id, instrument_id);
CREATE INDEX IF NOT EXISTS idx_budgets_tenant ON budgets(tenant_id);
CREATE INDEX IF NOT EXISTS idx_goals_tenant ON goals(tenant_id);
CREATE INDEX IF NOT EXISTS idx_income_sources_tenant ON income_sources(tenant_id);
CREATE INDEX IF NOT EXISTS idx_import_jobs_tenant ON import_jobs(tenant_id);
CREATE INDEX IF NOT EXISTS idx_import_rows_job ON import_rows(import_job_id);
CREATE INDEX IF NOT EXISTS idx_brokers_tenant ON brokers(tenant_id);
CREATE INDEX IF NOT EXISTS idx_backups_tenant ON backups(tenant_id);
CREATE INDEX IF NOT EXISTS idx_recurring_rules_tenant ON recurring_rules(tenant_id);
CREATE INDEX IF NOT EXISTS idx_goal_contrib_account ON goal_contributions(tenant_id, account_id);
CREATE INDEX IF NOT EXISTS idx_trades_transaction ON investment_trades(transaction_id);
CREATE INDEX IF NOT EXISTS idx_payments_transaction ON liability_payments(transaction_id);
CREATE INDEX IF NOT EXISTS idx_dividends_account ON dividends(tenant_id, account_id);
CREATE INDEX IF NOT EXISTS idx_goal_milestones_goal ON goal_milestones(goal_id);
