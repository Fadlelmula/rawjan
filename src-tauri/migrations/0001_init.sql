-- Spiderlily Finance — schema v1
-- Conventions:
--   * Amounts are integer minor units (e.g. QAR 12.34 -> 1234).
--   * Rates are decimal strings; 1 unit of base_code = rate units of quote_code.
--   * Timestamps are ISO-8601 UTC. Financial periods use local_date (YYYY-MM-DD).
--   * Posted financial records are never deleted; they are reversed.
--   * tenant_id/user_id exist on every financial table for future multi-user support.

CREATE TABLE tenants (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  created_at  TEXT NOT NULL
) STRICT;

CREATE TABLE users (
  id             TEXT PRIMARY KEY,
  tenant_id      TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  display_name   TEXT NOT NULL,
  email          TEXT,
  is_local_owner INTEGER NOT NULL DEFAULT 1 CHECK (is_local_owner IN (0,1)),
  created_at     TEXT NOT NULL
) STRICT;
CREATE INDEX idx_users_tenant ON users(tenant_id);

CREATE TABLE currencies (
  code      TEXT PRIMARY KEY,
  name      TEXT NOT NULL,
  symbol    TEXT NOT NULL,
  exponent  INTEGER NOT NULL CHECK (exponent BETWEEN 0 AND 4),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1))
) STRICT;

CREATE TABLE tenant_settings (
  tenant_id  TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  key        TEXT NOT NULL,
  value      TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_id, key)
) STRICT;

-- ---------------------------------------------------------------------------
-- Exchange rates (immutable snapshots; never overwritten by newer rates)
-- ---------------------------------------------------------------------------
CREATE TABLE exchange_rates (
  id          TEXT PRIMARY KEY,
  tenant_id   TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  base_code   TEXT NOT NULL REFERENCES currencies(code),
  quote_code  TEXT NOT NULL REFERENCES currencies(code),
  rate        TEXT NOT NULL,
  as_of_date  TEXT NOT NULL,
  source      TEXT NOT NULL,
  fetched_at  TEXT NOT NULL,
  is_manual   INTEGER NOT NULL DEFAULT 0 CHECK (is_manual IN (0,1)),
  UNIQUE (tenant_id, base_code, quote_code, as_of_date, source)
) STRICT;
CREATE INDEX idx_fx_lookup ON exchange_rates(tenant_id, base_code, quote_code, as_of_date);

-- ---------------------------------------------------------------------------
-- Parties that hold investment accounts
-- ---------------------------------------------------------------------------
CREATE TABLE brokers (
  id         TEXT PRIMARY KEY,
  tenant_id  TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  kind       TEXT NOT NULL DEFAULT 'broker' CHECK (kind IN ('broker','bank','platform','exchange','other')),
  website    TEXT,
  notes      TEXT,
  created_at TEXT NOT NULL
) STRICT;

-- ---------------------------------------------------------------------------
-- Accounts
-- ---------------------------------------------------------------------------
CREATE TABLE accounts (
  id                    TEXT PRIMARY KEY,
  tenant_id             TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id               TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name                  TEXT NOT NULL,
  type                  TEXT NOT NULL CHECK (type IN ('cash','bank','debit_card','credit_card','savings','wallet','investment','loan','other')),
  institution           TEXT,
  currency_code         TEXT NOT NULL REFERENCES currencies(code),
  is_liability          INTEGER NOT NULL DEFAULT 0 CHECK (is_liability IN (0,1)),
  is_liquid             INTEGER NOT NULL DEFAULT 1 CHECK (is_liquid IN (0,1)),
  opening_balance_minor INTEGER NOT NULL DEFAULT 0,
  opening_date          TEXT,
  credit_limit_minor    INTEGER,
  statement_day         INTEGER CHECK (statement_day BETWEEN 1 AND 31),
  payment_due_day       INTEGER CHECK (payment_due_day BETWEEN 1 AND 31),
  broker_id             TEXT REFERENCES brokers(id) ON DELETE SET NULL,
  notes                 TEXT,
  color                 TEXT,
  sort_order            INTEGER NOT NULL DEFAULT 0,
  is_active             INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at            TEXT NOT NULL,
  updated_at            TEXT NOT NULL,
  archived_at           TEXT
) STRICT;
CREATE INDEX idx_accounts_tenant ON accounts(tenant_id, is_active, sort_order);

-- ---------------------------------------------------------------------------
-- Categories and tags
-- ---------------------------------------------------------------------------
CREATE TABLE categories (
  id         TEXT PRIMARY KEY,
  tenant_id  TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  parent_id  TEXT REFERENCES categories(id) ON DELETE RESTRICT,
  name       TEXT NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('income','expense','equity')),
  icon       TEXT,
  color      TEXT,
  is_system  INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0,1)),
  is_archived INTEGER NOT NULL DEFAULT 0 CHECK (is_archived IN (0,1)),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
) STRICT;
CREATE UNIQUE INDEX idx_categories_unique ON categories(tenant_id, COALESCE(parent_id, ''), name);
CREATE INDEX idx_categories_kind ON categories(tenant_id, kind, is_archived);

CREATE TABLE tags (
  id         TEXT PRIMARY KEY,
  tenant_id  TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  color      TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (tenant_id, name)
) STRICT;

-- ---------------------------------------------------------------------------
-- Income sources
-- ---------------------------------------------------------------------------
CREATE TABLE income_sources (
  id                   TEXT PRIMARY KEY,
  tenant_id            TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id              TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name                 TEXT NOT NULL,
  type                 TEXT NOT NULL CHECK (type IN ('salary','bonus','business','freelance','dividend','rental','interest','gift','other')),
  currency_code        TEXT NOT NULL REFERENCES currencies(code),
  expected_amount_minor INTEGER NOT NULL DEFAULT 0,
  schedule             TEXT NOT NULL DEFAULT 'monthly' CHECK (schedule IN ('monthly','semimonthly','biweekly','weekly','irregular','one_time')),
  pay_day              INTEGER CHECK (pay_day BETWEEN 1 AND 31),
  account_id           TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  category_id          TEXT REFERENCES categories(id) ON DELETE SET NULL,
  is_active            INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  notes                TEXT,
  created_at           TEXT NOT NULL,
  updated_at           TEXT NOT NULL
) STRICT;

-- ---------------------------------------------------------------------------
-- Recurring rules
-- ---------------------------------------------------------------------------
CREATE TABLE recurring_rules (
  id             TEXT PRIMARY KEY,
  tenant_id      TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id        TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name           TEXT NOT NULL,
  frequency      TEXT NOT NULL CHECK (frequency IN ('daily','weekly','biweekly','monthly','yearly')),
  interval_count INTEGER NOT NULL DEFAULT 1 CHECK (interval_count >= 1),
  day_of_month   INTEGER CHECK (day_of_month BETWEEN 1 AND 31),
  weekday        INTEGER CHECK (weekday BETWEEN 0 AND 6),
  start_date     TEXT NOT NULL,
  end_date       TEXT,
  next_run_date  TEXT NOT NULL,
  posting_mode   TEXT NOT NULL DEFAULT 'pending' CHECK (posting_mode IN ('auto','pending')),
  template_json  TEXT NOT NULL,
  is_active      INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  last_posted_at TEXT,
  created_at     TEXT NOT NULL
) STRICT;

CREATE TABLE recurring_exceptions (
  id             TEXT PRIMARY KEY,
  tenant_id      TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  rule_id        TEXT NOT NULL REFERENCES recurring_rules(id) ON DELETE CASCADE,
  exception_date TEXT NOT NULL,
  action         TEXT NOT NULL CHECK (action IN ('skip','shift')),
  shifted_to     TEXT,
  note           TEXT,
  created_at     TEXT NOT NULL,
  UNIQUE (rule_id, exception_date)
) STRICT;

-- ---------------------------------------------------------------------------
-- Import jobs
-- ---------------------------------------------------------------------------
CREATE TABLE import_jobs (
  id              TEXT PRIMARY KEY,
  tenant_id       TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  file_name       TEXT NOT NULL,
  file_hash       TEXT NOT NULL,
  format          TEXT NOT NULL CHECK (format IN ('csv','xlsx','ofx','qif','pdf','json')),
  status          TEXT NOT NULL CHECK (status IN ('parsed','committed','failed','cancelled')),
  account_id      TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  mapping_json    TEXT,
  row_count       INTEGER NOT NULL DEFAULT 0,
  imported_count  INTEGER NOT NULL DEFAULT 0,
  duplicate_count INTEGER NOT NULL DEFAULT 0,
  error_count     INTEGER NOT NULL DEFAULT 0,
  created_at      TEXT NOT NULL,
  committed_at    TEXT
) STRICT;

-- ---------------------------------------------------------------------------
-- Transactions and double-entry ledger
-- ---------------------------------------------------------------------------
CREATE TABLE transactions (
  id                  TEXT PRIMARY KEY,
  tenant_id           TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id             TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  kind                TEXT NOT NULL CHECK (kind IN ('expense','income','transfer','refund','fee','investment_buy','investment_sell','dividend','adjustment','opening')),
  status              TEXT NOT NULL CHECK (status IN ('draft','pending','posted','reconciled','cancelled','reversed')),
  data_status         TEXT NOT NULL DEFAULT 'actual' CHECK (data_status IN ('actual','pending','planned','estimated','forecast')),
  description         TEXT NOT NULL,
  payee               TEXT,
  notes               TEXT,
  local_date          TEXT NOT NULL,
  occurred_at         TEXT NOT NULL,
  account_id          TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  counter_account_id  TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
  category_id         TEXT REFERENCES categories(id) ON DELETE RESTRICT,
  income_source_id    TEXT REFERENCES income_sources(id) ON DELETE SET NULL,
  transfer_group_id   TEXT,
  reversal_of_id      TEXT REFERENCES transactions(id) ON DELETE RESTRICT,
  recurring_rule_id   TEXT REFERENCES recurring_rules(id) ON DELETE SET NULL,
  import_job_id       TEXT REFERENCES import_jobs(id) ON DELETE SET NULL,
  external_ref        TEXT,
  original_amount_minor INTEGER NOT NULL CHECK (original_amount_minor >= 0),
  original_currency   TEXT NOT NULL REFERENCES currencies(code),
  fx_rate_id          TEXT REFERENCES exchange_rates(id) ON DELETE RESTRICT,
  counter_amount_minor INTEGER CHECK (counter_amount_minor IS NULL OR counter_amount_minor >= 0),
  counter_currency    TEXT REFERENCES currencies(code),
  base_amount_minor   INTEGER NOT NULL CHECK (base_amount_minor >= 0),
  base_currency       TEXT NOT NULL REFERENCES currencies(code),
  is_split            INTEGER NOT NULL DEFAULT 0 CHECK (is_split IN (0,1)),
  dedupe_hash         TEXT,
  created_at          TEXT NOT NULL,
  updated_at          TEXT NOT NULL,
  posted_at           TEXT,
  cancelled_at        TEXT,
  reconciled_at       TEXT
) STRICT;
CREATE INDEX idx_tx_date ON transactions(tenant_id, local_date DESC);
CREATE INDEX idx_tx_account_date ON transactions(tenant_id, account_id, local_date DESC);
CREATE INDEX idx_tx_category ON transactions(tenant_id, category_id, local_date);
CREATE INDEX idx_tx_kind_status ON transactions(tenant_id, kind, status, local_date);
CREATE INDEX idx_tx_recurring ON transactions(recurring_rule_id, local_date);
CREATE UNIQUE INDEX idx_tx_recurring_once ON transactions(recurring_rule_id, local_date)
  WHERE recurring_rule_id IS NOT NULL AND status <> 'cancelled';
CREATE INDEX idx_tx_dedupe ON transactions(tenant_id, account_id, dedupe_hash);
CREATE INDEX idx_tx_import ON transactions(import_job_id);
CREATE INDEX idx_tx_reversal ON transactions(reversal_of_id);

CREATE TABLE ledger_entries (
  id               TEXT PRIMARY KEY,
  tenant_id        TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  transaction_id   TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  account_id       TEXT REFERENCES accounts(id) ON DELETE RESTRICT,
  category_id      TEXT REFERENCES categories(id) ON DELETE RESTRICT,
  entry_type       TEXT NOT NULL CHECK (entry_type IN ('debit','credit')),
  amount_minor     INTEGER NOT NULL CHECK (amount_minor >= 0),
  currency_code    TEXT NOT NULL REFERENCES currencies(code),
  base_amount_minor INTEGER NOT NULL CHECK (base_amount_minor >= 0),
  base_currency    TEXT NOT NULL REFERENCES currencies(code),
  fx_rate_id       TEXT REFERENCES exchange_rates(id) ON DELETE RESTRICT,
  memo             TEXT,
  created_at       TEXT NOT NULL,
  CHECK ((account_id IS NOT NULL AND category_id IS NULL) OR (account_id IS NULL AND category_id IS NOT NULL))
) STRICT;
CREATE INDEX idx_ledger_tx ON ledger_entries(transaction_id);
CREATE INDEX idx_ledger_account ON ledger_entries(tenant_id, account_id);
CREATE INDEX idx_ledger_category ON ledger_entries(tenant_id, category_id);

CREATE TABLE transaction_splits (
  id                TEXT PRIMARY KEY,
  tenant_id         TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  transaction_id    TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  category_id       TEXT NOT NULL REFERENCES categories(id) ON DELETE RESTRICT,
  amount_minor      INTEGER NOT NULL CHECK (amount_minor > 0),
  base_amount_minor INTEGER NOT NULL CHECK (base_amount_minor >= 0),
  note              TEXT,
  sort_order        INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE INDEX idx_splits_tx ON transaction_splits(transaction_id);

CREATE TABLE transaction_tags (
  transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  tag_id         TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  tenant_id      TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  PRIMARY KEY (transaction_id, tag_id)
) STRICT;

CREATE TABLE transaction_attachments (
  id             TEXT PRIMARY KEY,
  tenant_id      TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  file_name      TEXT NOT NULL,
  mime_type      TEXT NOT NULL,
  byte_size      INTEGER NOT NULL,
  sha256         TEXT NOT NULL,
  storage_path   TEXT NOT NULL,
  created_at     TEXT NOT NULL
) STRICT;
CREATE INDEX idx_attachments_tx ON transaction_attachments(transaction_id);

CREATE TABLE categorization_history (
  id                   TEXT PRIMARY KEY,
  tenant_id            TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  transaction_id       TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  rule_id              TEXT,
  previous_category_id TEXT,
  new_category_id      TEXT,
  actor                TEXT NOT NULL CHECK (actor IN ('rule','user','import')),
  created_at           TEXT NOT NULL
) STRICT;

-- ---------------------------------------------------------------------------
-- Budgets
-- ---------------------------------------------------------------------------
CREATE TABLE budgets (
  id              TEXT PRIMARY KEY,
  tenant_id       TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name            TEXT NOT NULL,
  method          TEXT NOT NULL DEFAULT 'custom' CHECK (method IN ('custom','zero_based','envelope','fifty_thirty_twenty')),
  period_type     TEXT NOT NULL DEFAULT 'cycle_month' CHECK (period_type IN ('calendar_month','cycle_month','weekly','daily','custom')),
  cycle_start_day INTEGER NOT NULL DEFAULT 28 CHECK (cycle_start_day BETWEEN 1 AND 31),
  rollover_policy TEXT NOT NULL DEFAULT 'none' CHECK (rollover_policy IN ('none','unspent','full')),
  currency_code   TEXT NOT NULL REFERENCES currencies(code),
  alert_thresholds TEXT NOT NULL DEFAULT '[50,75,90,100]',
  is_active       INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  start_date      TEXT NOT NULL,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
) STRICT;

CREATE TABLE budget_allocations (
  id          TEXT PRIMARY KEY,
  tenant_id   TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  budget_id   TEXT NOT NULL REFERENCES budgets(id) ON DELETE CASCADE,
  category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE RESTRICT,
  amount_minor INTEGER NOT NULL CHECK (amount_minor >= 0),
  notes       TEXT,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  UNIQUE (budget_id, category_id)
) STRICT;

CREATE TABLE budget_periods (
  id              TEXT PRIMARY KEY,
  tenant_id       TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  budget_id       TEXT NOT NULL REFERENCES budgets(id) ON DELETE CASCADE,
  start_date      TEXT NOT NULL,
  end_date        TEXT NOT NULL,
  allocated_minor INTEGER NOT NULL,
  spent_minor     INTEGER NOT NULL,
  rolled_in_minor INTEGER NOT NULL DEFAULT 0,
  rolled_out_minor INTEGER NOT NULL DEFAULT 0,
  is_closed       INTEGER NOT NULL DEFAULT 0 CHECK (is_closed IN (0,1)),
  computed_at     TEXT NOT NULL,
  UNIQUE (budget_id, start_date)
) STRICT;

-- ---------------------------------------------------------------------------
-- Savings goals
-- ---------------------------------------------------------------------------
CREATE TABLE goals (
  id                        TEXT PRIMARY KEY,
  tenant_id                 TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id                   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name                      TEXT NOT NULL,
  target_amount_minor       INTEGER NOT NULL CHECK (target_amount_minor > 0),
  currency_code             TEXT NOT NULL REFERENCES currencies(code),
  target_date               TEXT,
  priority                  INTEGER NOT NULL DEFAULT 3 CHECK (priority BETWEEN 1 AND 5),
  account_id                TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  monthly_contribution_minor INTEGER NOT NULL DEFAULT 0,
  start_date                TEXT NOT NULL,
  status                    TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','achieved','paused','archived')),
  notes                     TEXT,
  created_at                TEXT NOT NULL,
  updated_at                TEXT NOT NULL
) STRICT;

CREATE TABLE goal_contributions (
  id             TEXT PRIMARY KEY,
  tenant_id      TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  goal_id        TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  amount_minor   INTEGER NOT NULL CHECK (amount_minor > 0),
  contributed_on TEXT NOT NULL,
  account_id     TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
  note           TEXT,
  created_at     TEXT NOT NULL
) STRICT;
CREATE INDEX idx_goal_contrib ON goal_contributions(goal_id, contributed_on);

CREATE TABLE goal_milestones (
  id                 TEXT PRIMARY KEY,
  tenant_id          TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  goal_id            TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  name               TEXT NOT NULL,
  target_amount_minor INTEGER NOT NULL,
  reached_at         TEXT
) STRICT;

-- ---------------------------------------------------------------------------
-- Investments
-- ---------------------------------------------------------------------------
CREATE TABLE instruments (
  id          TEXT PRIMARY KEY,
  tenant_id   TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  symbol      TEXT NOT NULL,
  name        TEXT NOT NULL,
  asset_class TEXT NOT NULL CHECK (asset_class IN ('stock','etf','bond','gold','silver','crypto','real_estate','cash_deposit','other')),
  currency_code TEXT NOT NULL REFERENCES currencies(code),
  exchange    TEXT,
  isin        TEXT,
  notes       TEXT,
  created_at  TEXT NOT NULL,
  UNIQUE (tenant_id, symbol, exchange)
) STRICT;

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
  price_scale   INTEGER NOT NULL DEFAULT 8,
  fee_minor     INTEGER NOT NULL DEFAULT 0,
  currency_code TEXT NOT NULL REFERENCES currencies(code),
  fx_rate_id    TEXT REFERENCES exchange_rates(id) ON DELETE RESTRICT,
  transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
  notes         TEXT,
  created_at    TEXT NOT NULL
) STRICT;
CREATE INDEX idx_trades_account ON investment_trades(tenant_id, account_id, trade_date);
CREATE INDEX idx_trades_instrument ON investment_trades(tenant_id, instrument_id, trade_date);

CREATE TABLE investment_valuations (
  id            TEXT PRIMARY KEY,
  tenant_id     TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  account_id    TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  instrument_id TEXT REFERENCES instruments(id) ON DELETE CASCADE,
  as_of_date    TEXT NOT NULL,
  quantity_scaled INTEGER NOT NULL DEFAULT 0,
  price_scaled  INTEGER NOT NULL DEFAULT 0,
  price_scale   INTEGER NOT NULL DEFAULT 8,
  value_minor   INTEGER NOT NULL,
  currency_code TEXT NOT NULL REFERENCES currencies(code),
  source        TEXT NOT NULL CHECK (source IN ('manual','market_api','import','statement')),
  source_ref    TEXT,
  created_at    TEXT NOT NULL
) STRICT;
CREATE INDEX idx_valuations ON investment_valuations(tenant_id, account_id, as_of_date);

CREATE TABLE dividends (
  id            TEXT PRIMARY KEY,
  tenant_id     TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  account_id    TEXT NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
  instrument_id TEXT REFERENCES instruments(id) ON DELETE SET NULL,
  pay_date      TEXT NOT NULL,
  gross_minor   INTEGER NOT NULL CHECK (gross_minor >= 0),
  tax_minor     INTEGER NOT NULL DEFAULT 0,
  currency_code TEXT NOT NULL REFERENCES currencies(code),
  transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
  notes         TEXT,
  created_at    TEXT NOT NULL
) STRICT;

-- ---------------------------------------------------------------------------
-- Liabilities
-- ---------------------------------------------------------------------------
CREATE TABLE liabilities (
  id                TEXT PRIMARY KEY,
  tenant_id         TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id           TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name              TEXT NOT NULL,
  type              TEXT NOT NULL CHECK (type IN ('credit_card','loan','installment','money_lent','money_borrowed','other')),
  account_id        TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  principal_minor   INTEGER NOT NULL CHECK (principal_minor >= 0),
  outstanding_minor INTEGER NOT NULL CHECK (outstanding_minor >= 0),
  currency_code     TEXT NOT NULL REFERENCES currencies(code),
  interest_rate_bps INTEGER NOT NULL DEFAULT 0 CHECK (interest_rate_bps >= 0),
  start_date        TEXT,
  term_months       INTEGER CHECK (term_months > 0),
  payment_minor     INTEGER NOT NULL DEFAULT 0,
  due_day           INTEGER CHECK (due_day BETWEEN 1 AND 31),
  first_payment_date TEXT,
  status            TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','paid','defaulted','archived')),
  notes             TEXT,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL
) STRICT;

CREATE TABLE liability_payments (
  id             TEXT PRIMARY KEY,
  tenant_id      TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  liability_id   TEXT NOT NULL REFERENCES liabilities(id) ON DELETE CASCADE,
  paid_on        TEXT NOT NULL,
  amount_minor   INTEGER NOT NULL CHECK (amount_minor > 0),
  principal_minor INTEGER NOT NULL DEFAULT 0,
  interest_minor INTEGER NOT NULL DEFAULT 0,
  fee_minor      INTEGER NOT NULL DEFAULT 0,
  transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
  note           TEXT,
  created_at     TEXT NOT NULL
) STRICT;
CREATE INDEX idx_liability_payments ON liability_payments(liability_id, paid_on);

-- ---------------------------------------------------------------------------
-- Net worth snapshots
-- ---------------------------------------------------------------------------
CREATE TABLE net_worth_snapshots (
  id              TEXT PRIMARY KEY,
  tenant_id       TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  as_of_date      TEXT NOT NULL,
  currency_code   TEXT NOT NULL REFERENCES currencies(code),
  assets_minor    INTEGER NOT NULL,
  liabilities_minor INTEGER NOT NULL,
  net_minor       INTEGER NOT NULL,
  cash_minor      INTEGER NOT NULL DEFAULT 0,
  invested_minor  INTEGER NOT NULL DEFAULT 0,
  breakdown_json  TEXT NOT NULL,
  source          TEXT NOT NULL DEFAULT 'computed',
  created_at      TEXT NOT NULL,
  UNIQUE (tenant_id, as_of_date)
) STRICT;

-- ---------------------------------------------------------------------------
-- Notifications
-- ---------------------------------------------------------------------------
CREATE TABLE notifications (
  id                 TEXT PRIMARY KEY,
  tenant_id          TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  type               TEXT NOT NULL,
  title              TEXT NOT NULL,
  body               TEXT NOT NULL,
  severity           TEXT NOT NULL DEFAULT 'info' CHECK (severity IN ('info','warning','critical')),
  related_entity_type TEXT,
  related_entity_id  TEXT,
  scheduled_for      TEXT,
  read_at            TEXT,
  created_at         TEXT NOT NULL
) STRICT;
CREATE INDEX idx_notifications ON notifications(tenant_id, created_at DESC);

CREATE TABLE notification_preferences (
  tenant_id  TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  type       TEXT NOT NULL,
  enabled    INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)),
  config_json TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_id, type)
) STRICT;

-- ---------------------------------------------------------------------------
-- Journal
-- ---------------------------------------------------------------------------
CREATE TABLE journal_entries (
  id         TEXT PRIMARY KEY,
  tenant_id  TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title      TEXT NOT NULL,
  body       TEXT NOT NULL,
  entry_date TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
) STRICT;
CREATE INDEX idx_journal_date ON journal_entries(tenant_id, entry_date DESC);

CREATE TABLE journal_links (
  id             TEXT PRIMARY KEY,
  tenant_id      TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  journal_id     TEXT NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
  entity_type    TEXT NOT NULL,
  entity_id      TEXT NOT NULL
) STRICT;

-- ---------------------------------------------------------------------------
-- Import rows and categorization rules
-- ---------------------------------------------------------------------------
CREATE TABLE import_rows (
  id              TEXT PRIMARY KEY,
  tenant_id       TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  import_job_id   TEXT NOT NULL REFERENCES import_jobs(id) ON DELETE CASCADE,
  row_index       INTEGER NOT NULL,
  raw_json        TEXT NOT NULL,
  normalized_json TEXT,
  status          TEXT NOT NULL CHECK (status IN ('ok','duplicate','error','skipped','imported')),
  error_message   TEXT,
  transaction_id  TEXT REFERENCES transactions(id) ON DELETE SET NULL
) STRICT;
CREATE INDEX idx_import_rows ON import_rows(import_job_id, row_index);

CREATE TABLE import_rules (
  id               TEXT PRIMARY KEY,
  tenant_id        TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  name             TEXT NOT NULL,
  priority         INTEGER NOT NULL DEFAULT 100,
  match_field      TEXT NOT NULL CHECK (match_field IN ('description','payee','amount','account')),
  match_type       TEXT NOT NULL CHECK (match_type IN ('contains','starts_with','equals','regex')),
  match_value      TEXT NOT NULL,
  set_category_id  TEXT REFERENCES categories(id) ON DELETE SET NULL,
  set_payee        TEXT,
  is_active        INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at       TEXT NOT NULL
) STRICT;

-- ---------------------------------------------------------------------------
-- Audit and backups
-- ---------------------------------------------------------------------------
CREATE TABLE audit_events (
  id           TEXT PRIMARY KEY,
  tenant_id    TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id      TEXT,
  entity_type  TEXT NOT NULL,
  entity_id    TEXT NOT NULL,
  action       TEXT NOT NULL,
  before_json  TEXT,
  after_json   TEXT,
  created_at   TEXT NOT NULL
) STRICT;
CREATE INDEX idx_audit_entity ON audit_events(tenant_id, entity_type, entity_id, created_at DESC);

CREATE TABLE backups (
  id          TEXT PRIMARY KEY,
  tenant_id   TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  path        TEXT NOT NULL,
  kind        TEXT NOT NULL CHECK (kind IN ('manual','daily','pre_migration','export')),
  byte_size   INTEGER NOT NULL,
  sha256      TEXT NOT NULL,
  encrypted   INTEGER NOT NULL DEFAULT 1 CHECK (encrypted IN (0,1)),
  schema_version INTEGER NOT NULL,
  verified_at TEXT,
  created_at  TEXT NOT NULL
) STRICT;

-- ---------------------------------------------------------------------------
-- Reconciliation
-- ---------------------------------------------------------------------------
CREATE TABLE reconciliation_sessions (
  id                     TEXT PRIMARY KEY,
  tenant_id              TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  account_id             TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  statement_start        TEXT NOT NULL,
  statement_end          TEXT NOT NULL,
  statement_balance_minor INTEGER NOT NULL,
  status                 TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','completed','abandoned')),
  completed_at           TEXT,
  created_at             TEXT NOT NULL
) STRICT;

CREATE TABLE reconciliation_items (
  id              TEXT PRIMARY KEY,
  tenant_id       TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  session_id      TEXT NOT NULL REFERENCES reconciliation_sessions(id) ON DELETE CASCADE,
  transaction_id  TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  included        INTEGER NOT NULL DEFAULT 1 CHECK (included IN (0,1)),
  UNIQUE (session_id, transaction_id)
) STRICT;
