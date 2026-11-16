# RAWJAN — COMPLETE APPLICATION AUDIT

Date: 2026-11-16 · Commit audited: `7d2b039` (clean working tree) · Auditor: automated deep audit, verified against source
Method: full-repository inspection (166 tracked files; all 33 frontend files and all 30 backend files read), calculation tracing from UI → Tauri command → service → SQL and back, and live execution of every test/lint/build command. No application code was modified.

Every "confirmed" issue below was re-verified against the exact file and line cited. Findings are labelled **CONFIRMED BUG**, **POTENTIAL BUG (needs verification)**, **DESIGN/ARCHITECTURE WEAKNESS**, **MISSING FUNCTIONALITY**, or **OPTIONAL IMPROVEMENT**.

---

## 1. Executive Summary

Rawjan is a genuinely well-built local-first finance product: a real double-entry ledger with integer minor-unit money, checked decimal math, parameterized SQL everywhere, SQLCipher encryption, honest UI copy, 119 passing Rust tests, and no telemetry. The five previously reported calculation defects were all fixed correctly in `7d2b039` and are now regression-tested.

The audit nonetheless found **one critical data-loss bug, three high-severity integrity/reliability defects, and a set of medium-severity issues** concentrated in three areas:

1. **Write atomicity.** The transaction engine (create/reverse) is properly transactional, but several multi-statement mutations are not. The worst is `update_transaction` on drafts/pending entries, which commits a delete and then re-creates the record in a separate step: if re-creation fails, the entry and its ledger rows are permanently lost (`tx.rs:1516-1543`). Liability payment recording and deletion have the same split-brain shape.
2. **Key handling edge cases.** A malformed Windows Credential Manager entry silently triggers generation of a *new* database key, overwriting the only key to the existing vault (`db.rs:75-88`).
3. **Display-layer honesty.** Several UI numbers are computed in JavaScript from money values (contradicting the app's own architecture claim), and in one place a bar chart disagrees with the percentage printed next to it. Dashboard count cards carry hints ("Active") that the underlying counts do not honor. Four pages show a fake "empty" state when a query fails.

Financial calculations themselves (ledger balance, income/expense aggregation, budgets, investments cost basis, net worth, FX conversion, payoff estimates) are correct as implemented and covered by tests. The remaining calculation risks are policy choices (semimonthly pay-date model, budget FX date, one-time receipt semantics) documented in §4 and §13.

**Bottom line:** the math is trustworthy; the *durability* of some edits and the *consistency of some displays* are not yet at release quality. Fix the RA-001…RA-005 items before wider distribution.

---

## 2. Application Architecture

- **Shell:** Tauri 2 (Windows/WebView2), per-user NSIS installer. Product identity: dark "red spider lily" design.
- **Frontend:** React 19 + TypeScript (strict) + Tailwind, hash routing (`react-router-dom`), TanStack Query for server state, Vite build (single ~609 kB JS chunk).
- **IPC:** 81 Tauri commands registered in `src-tauri/src/lib.rs:29-111`; typed wrappers in `src/lib/api.ts`. All money crosses the boundary as display strings plus `*Minor` integers; the UI is documented to never do financial arithmetic.
- **Backend:** thin `api/*` commands → `services/*` domain logic → SQL via `rusqlite`. One `Mutex<Connection>` in `state.rs:9-16`; `AppError` serialized as strings to the frontend (`error.rs:27-34`).
- **Database:** SQLCipher-encrypted SQLite, `STRICT` tables, WAL, foreign keys on, forward-only migration `migrations/0001_init.sql` (schema v1). Key: 256-bit random, held in Windows Credential Manager (`rawjan-finance`/`database-key`), overridable via `RAWJAN_DB_KEY`.
- **Money:** `i64` minor units; `rust_decimal` for rates/scaling; strict grammar parsers (`money.rs`); `checked_sum`/`checked_diff` overflow helpers exist but are applied inconsistently (see RA-005).
- **Ledger:** `ledger_entries` per transaction (debit/credit legs in base units), posted rows immutable, correction by reversal (`status='reversed'` + flipped counterpart legs, `tx.rs:1248-1418`).
- **Tests:** 119 Rust unit/integration tests (in-module), 10 Vitest tests (`src/lib/display.test.ts` only). No component or E2E tests, no CI.

---

## 3. Complete Feature Inventory

| Area | Where | State |
|---|---|---|
| Dashboard (net worth hero, counts, recent activity, allocation, module status) | `DashboardPage.tsx` | Working; count hints misleading (RA-011) |
| Transactions (kinds, splits, statuses, search, filters, paging, duplicate check, reversal, draft delete) | `TransactionsPage.tsx`, `TransactionComposer.tsx`, `TransactionDetail.tsx`, `services/tx.rs` | Working; draft edit data-loss bug (RA-001) |
| Accounts (7 types, per-currency, register, archive) | `AccountsPage.tsx`, `services/accounts.rs` | Working; register truncated at 100 rows (RA-042) |
| Income (report, sources, expected-vs-received, unassigned) | `IncomePage.tsx`, `services/income.rs` | Working; corrected in `7d2b039` (§13) |
| Expenses/refunds/fees | part of Transactions + budgets | Working |
| Transfers (incl. cross-currency with counter amount) | `services/tx.rs:249-307` | Working |
| Budgets (calendar/cycle/weekly/daily, allocations, FX, rollover policy) | `BudgetsPage.tsx`, `services/budgets.rs` | Working; rollover stored but never applied (RA-032) |
| Categories & tags | `api/mod.rs:69-104`, `api/accounts.rs` | Working |
| Recurring transactions | schema only (`recurring_rules`, `recurring_occurrences`, `0001_init.sql:154-187`) | **Not implemented** (RA-040) |
| Bills & subscriptions | — | **Absent** (only "due day"/"next due" on liabilities) |
| Savings goals (+ contributions, required-monthly estimate) | `GoalsPage.tsx`, `services/goals.rs` | Working; delete cascades history (RA-004) |
| Investments (instruments, trades, valuations, avg-cost basis, realized/unrealized) | `InvestmentsPage.tsx`, `services/investments.rs` | Working; valuation edit bug (RA-015) |
| Liabilities (loans/cards/money lent/borrowed, payments, payoff estimate) | `LiabilitiesPage.tsx`, `services/liabilities.rs` | Working; write atomicity bugs (RA-002) |
| Net worth (composition, snapshots, trend, missing-rate reporting) | `NetWorthPage.tsx`, `services/networth.rs`, `services/accounts.rs:441+` | Working; fixed valuation summing in `7d2b039` |
| Analytics (monthly flow, category slices, payees) | `AnalyticsPage.tsx`, `services/analytics.rs` | Working; bar/label mismatch (RA-012) |
| Calendar (per-day activity) | `CalendarPage.tsx`, `services/calendar.rs` | Working |
| Journal (notes, audit) | `JournalPage.tsx`, `services/journal.rs` | Working; bodies duplicated into audit trail (RA-009) |
| Financial health score | `HealthPage.tsx`, `services/health.rs` | Working; unreachable empty state (RA-030) |
| FI calculator / Financial lab (compound, loan, inflation) | `FiPage.tsx`, `LabPage.tsx`, `services/fi.rs`, `services/lab.rs` | Working |
| Import (CSV/XLSX/OFX/QIF, preview, mapping, dedupe) | `ImportsPage.tsx`, `services/imports.rs` | Working; non-atomic commit (RA-008), no confirm (RA-013) |
| Backups / encrypted exports / validation | `BackupsPage.tsx`, `services/backups.rs` | Working; export "verified" title bug (RA-029) |
| Settings (vault health, preferences, currencies/rates, integrations, audit trail) | `SettingsPage.tsx`, `api/mod.rs` | Working; scheduled-backups toggle inert (RA-032) |
| Notifications | — | **Absent** |
| Search & filtering | Transactions, Accounts, Journal | Working (LIKE wildcard quirk RA-024) |

No TODO/FIXME/mock/seed demo code exists anywhere in the tree (verified by repo-wide grep). All "not built" claims in README/UI are honest.

---

## 4. Critical Financial Calculation Findings

All calculations below were traced end-to-end and are **correct as implemented**, with regression tests where noted: ledger balance invariants (`tx.rs:542-548`, tests `tx.rs:1399-1447`); income/expense aggregation with refund netting and reversal exclusion (`analytics.rs:100-136, 138-199`); budget spend netting with FX-unavailable handling (`budgets.rs:116-143, 255-340`); investment average-cost basis, fee handling and realized P/L (`investments.rs:409-553`, test `:1280+`); net-worth composition incl. receivables and missing-rate exclusion (`accounts.rs:441-596`); FX rate selection (latest on-or-before date, inverse fallback, immutability — `currency.rs:62-132`, tests `:215-290`); liability payoff simulation (`liabilities.rs:247-276`); loan amortization (`lab.rs:198-285`).

**Policy choices a human should sign off** (not bugs, but financial meaning depends on them):

- **Semimonthly income model** (`income.rs:139-161`): payouts are placed at pay day and pay day+15 clamped to month end. If the user's real schedule is 1st/15th or 15th/last, expected totals differ. *Needs manual verification against the intended definition.*
- **Budget FX date** (`budgets.rs:270-290`): whole-period spend is converted at the rate on/before the period *end*. Rationale is defensible but should be confirmed.
- **One-time income** (`income.rs:163`): any receipt dated ≤ range end suppresses the expectation for that and all later ranges.
- **Monthly income without a pay day** (`income.rs:123-133`): counts one occurrence per touched month without an anchor date — an estimate, not a dated schedule.
- **Linked-liability net worth rule** (`accounts.rs:647-652` + receivables/debt queries): the liability record overrides the linked account entirely. Correct for preventing double counting, but confirm no real account relies on independent counting while linked.

---

## 5. Counting and Aggregation Findings

| Metric | Definition in code | Verdict |
|---|---|---|
| Income entries (income report) | `kind IN ('income','dividend')`, posted/reconciled, non-reversed (`income.rs:451`) | Correct (fixed in `7d2b039`) |
| Month income/expense/general counts | Split by kind (`analytics.rs:120-125`) | Correct |
| Analytics `transactionCount` | all effective rows incl. transfers (`analytics.rs:317`) | Correct per label "entries"; differs from income count by design |
| Calendar day `transaction_count` | all effective rows incl. transfers (`calendar.rs:73`) | Correct; semantics differ from day income/expense sums (transfers counted but not summed) — INFORMATIONAL |
| Category slice `transactionCount` | `COUNT(DISTINCT t.id)` of category legs (`analytics.rs:167`) | Correct (splits counted once) |
| Category percent | full eligible total before 12-row limit (`analytics.rs:185-196`) | Correct (fixed in `7d2b039`) |
| Expected income occurrences | see §4 | Correct with policy caveats |
| Dashboard "Transactions / Accounts / Budgets / Goals" | `COUNT(*)` per table (`api/mod.rs:258-267`) | **Misleading** — RA-011 |
| Payee counts | effective expense/fee rows (`analytics.rs:205-229`) | Correct |
| Import counts (imported/dupes/errors) | per-row outcomes (`imports.rs:551-633`) | Correct except crash scenario — RA-008 |

No double counting of the same transaction across income/expense/transfer categories was found. Reversed rows are consistently excluded from reports and included in balances (which nets to zero) — consistent by design.

---

## 6. Database and Data Integrity Findings

Schema is STRICT, FK-enforced, and well-chosen. Confirmed gaps:

- **RA-004** cascade deletes destroy financial history (see §4 issue list below).
- **RA-016** missing CHECK/UNIQUE constraints (see list).
- **RA-020** unindexed foreign keys (see list).
- Write atomicity defects RA-001/002/006/007/008 (see list).
- Transactions used correctly in the core engine: `create_transaction_inner` (`tx.rs:738-814`), `reverse_transaction` (`tx.rs:1335-1405`), `save_budget` (`budgets.rs:426-509`), import preview rows (`imports.rs:296-325`), migration (`db.rs:57-60`). ✔
- Audit-after-write pattern (RA-025): mutations return `Err` to the user if the audit insert fails although the write committed.

---

## 7. Backend Findings

Business logic reviewed service by service. Beyond the defects listed in §4/§6: validation is consistent and backend-enforced (amounts strict-parsed, negatives rejected where invalid, "paid requires zero outstanding" enforced at `liabilities.rs:582-585` and `:981-984` with test `:1314`); no silent error swallowing was found except the display-layer cases (RA-014) and the audit-after-write pattern (RA-025). Duplicated logic exists in percent helpers (`analytics.rs:90-95`, `income.rs:100-105`, `budgets.rs:84-89`, `goals.rs` inline) — minor duplication of an integer formula, acceptable but centralizing in `money.rs` would be cheap. Dead code and unused dependencies are listed under RA-034.

---

## 8. Frontend and UI/UX Findings

Highlights (full list captured in issue IDs):

- **Architecture violation:** money values are added/summed/percentaged in JS across Dashboard, Net worth, Analytics, Accounts, FI/Lab/Income charts (RA-019). In Analytics this produces a visible contradiction (RA-012).
- **Silent failures:** four pages render fake empty states when queries fail (RA-014); five pages spin forever below an error alert (RA-018).
- **Confirmation discipline** is good (14 of 16 destructive actions use `window.confirm`) but missing for the single biggest bulk write (RA-013).
- **Accessibility:** 8 icon-only buttons without names, 3 unlabeled search inputs, tables missing `th scope` except Income, no modal focus trap, toasts without `aria-live` (RA-028). The Income page (`IncomePage.tsx:472-488`) shows the intended standard.
- **Formatting:** dates handled via three different pipelines, incl. UTC `slice(0,10)` displays that can show the wrong day (RA-031).
- Visual identity, information hierarchy and empty-state quality are consistently strong.

---

## 9. Security and Privacy Findings

- **SQL injection:** none. Every dynamic SQL string uses bound parameters or compile-time constants (verified for `tx.rs:900-982, 1205-1234`, `api/mod.rs:202-205`, `backups.rs:160-162`, `db.rs:18-20`).
- **XSS:** React escaping + CSP `script-src 'self'` (`tauri.conf.json:30`). No `dangerouslySetInnerHTML`/`innerHTML` found.
- **Logging:** five log statements total; none contain amounts, payees, descriptions or keys (`state.rs:51-55`, `db.rs:111-152`).
- **Crypto:** SQLCipher with hex-validated key; Argon2id (19 MiB, t=2, p=1) + ChaCha20-Poly1305 exports with random salt/nonce and `zeroize` on success paths (`backups.rs:464-537`). Passphrase never stored.
- **Confirmed weaknesses:** keyring key handling (RA-003/022), arbitrary-path command inputs (RA-010), settings/journal values duplicated into audit JSON (RA-009), export verification reads files without a size cap (RA-010).
- **Privacy posture:** no network access at runtime, no telemetry. Encrypted exports exclude audit events and user emails; full backups contain everything (encrypted at rest).

---

## 10. Import and Export Findings

**Imports** (CSV/XLSX/OFX/QIF): 64 MiB and 20 000-row caps (`imports.rs:19-21`), preview-before-commit with per-row issues, configurable mapping/date order/decimal separator, dedupe via `(account, date, amount, description, kind)` hash (`imports.rs:778-791`), strict amount parsing with decoration tolerance (`imports.rs:1362-1400`). Confirmed issues: non-atomic commit (RA-008), no commit confirmation (RA-013), XLSX zip-bomb exposure (RA-023). Partial imports are per-row transactional and re-run-safe via dedupe, but job state/counts can lie after a crash (RA-008).

**Exports**: full-table encrypted JSON dump (`backups.rs:26-55`), round-trip verified before success is reported (`backups.rs:560-567`), legacy `.aureqis` compatibility. Totals consistency with the UI holds because exports contain the raw rows, not recomputed aggregates. **Missing:** any CSV/report-style export of displayed reports (MISSING FUNCTIONALITY).

---

## 11. Testing and Coverage Findings

Executed suites are in §17. Coverage is strong for money parsing, ledger balance, reversals, budgets, income schedules, FX, investments, liabilities, net worth, imports and exports. **Missing critical tests:**

1. `update_transaction` draft edit failure path (would have caught RA-001).
2. `save_payment`/`delete_payment` failure atomicity (RA-002).
3. Wrapped-sum validation on payment components and splits (RA-005).
4. `delete_goal`/`delete_liability` history expectations (RA-004 semantics).
5. Duplicate `investment_valuations` per (account, instrument, date).
6. Keyring malformed-entry behavior (RA-003).
7. Import commit crash/resume behavior (RA-008).
8. Frontend: zero component tests; the RA-012 bar/label mismatch is exactly what a component test would catch.
9. Property tests beyond the two deterministic sweeps in `money.rs`/`analytics.rs` (split rounding across rates is thinly covered).
10. E2E smoke (launch → account → transaction → report) — absent.

---

## 12. Performance and Scalability Findings

- **N+1 queries** pervade list endpoints (RA-021): `list_transactions` does a `currencies` lookup per row (`tx.rs:983-1033`); `list_accounts` runs valuation + rate + exponent queries per account, with per-instrument loops (`accounts.rs:217-296, 128-139`); income sources run 4 correlated subqueries each over unindexed `transactions.income_source_id` (`income.rs:239-257`). With 10 000+ transactions the dashboard/ledger loads will degrade noticeably; below ~1 000 rows everything is instant.
- **Missing indexes** (RA-020) compound this; joins on `liabilities.account_id`, `transaction_splits.category_id`, `investment_valuations.instrument_id` etc. are full scans.
- **Single `Mutex<Connection>`** serializes all commands (`state.rs:9-16`) — correct for SQLite, irrelevant at single-user scale.
- **Memory:** XLSX import materializes the whole first sheet (`imports.rs:1083-1086`); export verification reads entire files unbounded (RA-010).
- **Bundle:** 609 kB single JS chunk (Vite warning); code-splitting would improve startup marginally.
- **Startup:** bootstrap does one net-worth computation plus per-account queries — fine now, same N+1 shape as above.

No premature optimization recommended; index additions + currency-meta caching are the two changes with real payoff at scale.

---

## 13. Previously Identified Issues — Verification

All five concerns were re-verified against commit `7d2b039`. **All five were real and are now fixed.**

| # | Original concern | Verdict | Evidence now | Regression test |
|---|---|---|---|---|
| 1 | Income counts included non-income rows | **Was a bug; fixed** | `income.rs:448-451` sums `income_count` only; counts split by kind at `analytics.rs:120-125` | `income_counts_exclude_expense_and_general_rows` (`income.rs` tests), `activity_counts_are_separated_by_kind` |
| 2 | Category % over top-12 only | **Was a bug; fixed** | total computed before `.take(CATEGORY_DISPLAY_LIMIT)` (`analytics.rs:185-196`, limit at `:107`) | `category_percentages_use_the_complete_total_before_limiting` (13 categories → 7%, previously 8%) |
| 3 | Budget FX fallback to unconverted amount | **Was a bug; fixed** | conversion failure yields `None` + `fx_unavailable` (`budgets.rs:270-290, 311, 337`); UI shows notice (`BudgetsPage.tsx:536-542`) | `budget_fx_conversion_failure_is_surfaced_not_substituted` |
| 4 | Semimonthly = 2 per touched month | **Was a bug; fixed** | dated occurrences checked against the range (`income.rs:139-161`) | `semimonthly_payouts_count_only_days_that_fall_in_the_range` |
| 5 | Future one-time receipts changed history | **Was a bug; fixed** | receipts bounded by `local_date <= ?3` (range end) (`income.rs:257, 163`) | `a_future_one_time_receipt_does_not_change_historical_ranges` |

---

## Issue List (confirmed, with evidence)

### RA-001 · CRITICAL · CONFIRMED BUG · Data loss on draft/pending edit
- **Feature:** Transactions (edit). **File:** `src-tauri/src/services/tx.rs`. **Function:** `update_transaction`. **Lines:** 1516-1543 (delete commits at 1531; recreate runs at 1535-1542).
- **Problem:** editing a draft/pending entry deletes the transaction, its ledger entries, splits and tags, commits that deletion, and then re-creates the record in a *separate* operation. If re-creation fails (account archived since creation, category deleted, missing FX rate on a changed date, any validation error in the new values), the original entry is gone with no rollback.
- **Example:** edit a pending 5 000 QAR entry and change its date to a day with no FX rate for a foreign-currency account → error shown → the entry no longer exists anywhere.
- **Expected:** edit is atomic; failure leaves the original untouched.
- **Fix:** run delete + `create_transaction_inner` inside one rusqlite transaction (pass the transaction handle in), or re-validate inputs *before* deleting.
- **Regression test:** force `create_transaction_inner` to fail after the delete (e.g. reference an archived account) and assert the original rows still exist.

### RA-002 · HIGH · CONFIRMED BUG · Liability payments not atomic
- **Feature:** Liabilities. **File:** `src-tauri/src/services/liabilities.rs`. **Functions:** `save_payment` (lines 818-880), `delete_payment` (lines 922-948). No `unchecked_transaction` exists anywhere in the file.
- **Problem:** the payment row and the `liabilities.outstanding_minor/status` update are separate autocommit statements. A failure between them records a payment without adjusting the balance (or removes a payment without restoring it).
- **Example:** record a 400 QAR payment; the UPDATE fails (disk I/O) → payment list shows 400 paid, outstanding still 1 000.
- **Expected:** both statements commit or neither.
- **Fix:** wrap both statements in one transaction (same pattern as `save_budget`).
- **Regression test:** inject failure on the second statement; assert neither change persisted.

### RA-003 · HIGH · CONFIRMED BUG · Malformed keyring entry silently rotates the vault key
- **Feature:** Vault/key management. **File:** `src-tauri/src/db.rs`. **Function:** `resolve_database_key`. **Lines:** 75-88 (length check at 77, regenerate at 78-84).
- **Problem:** a stored secret whose length ≠ 64 falls into the generate-new-key arm and **overwrites** the credential. The existing encrypted database then cannot be opened and its only key is destroyed. A 64-char non-hex secret passes this check and later fails in `open()` (`db.rs:15-16`) with a confusing internal error.
- **Example:** Credential Manager entry corrupted/truncated by another tool → next launch generates a new key → vault unopenable, "wrong key" behavior with no recovery path.
- **Expected:** never overwrite an existing key silently; if the stored secret is unusable, fail with an explicit recovery message.
- **Fix:** only generate when `Err(NoEntry)`; on malformed `Ok`, error out. Validate hex at read time too.
- **Regression test:** seed a 10-char secret; assert `resolve_database_key` errors and does not call `set_password`.

### RA-004 · HIGH · CONFIRMED BUG (destructive-by-design, poorly bounded) · Cascade deletes erase financial history
- **Feature:** Goals, Liabilities. **Files:** `src-tauri/migrations/0001_init.sql:388` (`goal_contributions.goal_id … ON DELETE CASCADE`), `:504` (`liability_payments.liability_id … ON DELETE CASCADE`); `src-tauri/src/services/goals.rs:582-586` (`delete_goal`); `src-tauri/src/services/liabilities.rs:1003-1012` (`delete_liability`).
- **Problem:** deleting a goal or liability erases its entire contribution/payment history. The UI confirms first (so not silent), but the app's stated invariant is that financial history is preserved and corrected, not deleted. A user who deletes and re-creates a liability loses all repayment history irreversibly.
- **Example:** delete a 36-month-old loan with 24 recorded payments → all payment records gone; totals and audit trail references dangle.
- **Expected:** history-preserving behavior (archive) or an explicit "this permanently deletes N payment records" gate; ideally payments remain with `liability_id` nullable or soft-delete.
- **Fix:** prefer soft-delete/archive; at minimum strengthen the confirm text to quantify deleted history and add an export prompt.
- **Regression test:** document current cascade semantics in a test so any change is intentional.

### RA-005 · MEDIUM · CONFIRMED BUG · Unchecked sums can bypass validation in release builds
- **Feature:** Transactions (splits), Liabilities (payments). **Files:** `src-tauri/src/services/tx.rs:415` (`total += split_minor`) vs check at `:423`; `src-tauri/src/services/liabilities.rs:767` (`principal + interest + fee != amount`).
- **Problem:** plain `i64` addition over user-parsed values (individually up to ~9.2e16). In release builds sums wrap; a wrapped sum can equal the target and pass the "must add up" check with internally inconsistent components. The codebase has `money::checked_sum/checked_diff` (`money.rs:169-183`) used in budgets/analytics/income but not here.
- **Example:** split amounts near `i64::MAX` whose wrapped total coincidentally equals the transaction amount → ledger rows with a nonsensical split breakdown.
- **Expected:** all component sums use checked arithmetic and overflow is an error.
- **Fix:** use `checked_sum` in `tx.rs:415/492` and `liabilities.rs:767/789/811`.
- **Regression test:** feed `i64::MAX`-scale components; expect `Validation`, not acceptance.

### RA-006 · MEDIUM · CONFIRMED BUG · Posted-path category edit not atomic
- `src-tauri/src/services/tx.rs:1455-1493` (`update_transaction`): three UPDATEs + one INSERT run without a transaction; partial failure desynchronizes `ledger_entries.category_id` and `transactions.category_id`. Fix: single transaction. Test: fail mid-sequence, assert consistency.

### RA-007 · MEDIUM · CONFIRMED BUG · Account creation with opening balance not atomic
- `src-tauri/src/services/accounts.rs:429-468` (`create_account`): account INSERT commits before the opening transaction; error triggers a compensation DELETE, but a crash in between leaves an account whose `opening_balance_minor` is never reflected in the ledger. Fix: one transaction.

### RA-008 · MEDIUM · CONFIRMED BUG · Import commit is not atomic and its state can lie
- `src-tauri/src/services/imports.rs:557-633` (`commit`): per-row commits + final job UPDATE are unguarded; a crash mid-import leaves rows imported while the job still reads `status='parsed'` and counts are lost. Re-commit is mostly safe (dedupe at `:778-791`) but reports wrong numbers. Fix: single transaction per batch or resilient job state ("committing", resumable).

### RA-009 · MEDIUM · CONFIRMED BUG (conditional exposure) · Settings/journal values duplicated into audit JSON
- `src-tauri/src/api/mod.rs:387` (`_ => {}` accepts arbitrary setting keys) and `:390-401` (audit stores full before/after values); `src-tauri/src/services/journal.rs:107-141` stores full journal bodies in audit rows. Audit rows appear in the Audit-trail UI and in full backups. Today no secret is stored in settings, but the seeded `app_lock` key is an object field where a future PIN/hash would land in cleartext. Fix: allow-list setting keys; redact/omit values for designated keys; journal audits should record metadata only.

### RA-010 · MEDIUM · CONFIRMED BUG (defense-in-depth) · Frontend-supplied arbitrary file paths
- `src-tauri/src/services/imports.rs:187-360` (`import_preview` returns file content to the caller), `src-tauri/src/services/backups.rs:350-363` (`validate_backup`), `:613-621` (`verify_encrypted_export`, unbounded `fs::read` at `:620`). Any code able to invoke Tauri commands gets a read primitive; CSP mitigates XSS-driven abuse. Fix: restrict readable paths to the data dir + user-selected files (Tauri dialog tokens), cap verify size.

### RA-011 · MEDIUM · CONFIRMED BUG · Dashboard count hints don't match the counts
- `src-tauri/src/api/mod.rs:258-267` counts **all** rows (archived accounts, paused budgets, completed goals, drafts, reversals) while `src/pages/DashboardPage.tsx:180-207` labels them "Accounts — Active", "Budgets — Active", "Goals — In progress". Fix: filter counts by state or fix the hints. Test: archived account increments "Active" count — demonstrating the bug today.

### RA-012 · MEDIUM · CONFIRMED BUG · Analytics bars disagree with their printed percentages
- `src/pages/AnalyticsPage.tsx:78` (JS sum of displayed slices), `:293/315` (bar width from that sum), `:318` (label = Rust percent of full total), `:326` (footer claims Rust percentages). With 13 equal categories: labels read 7% while bars are drawn at 8.3%. Fix: drive bar width from `slice.percent`, or add a Rust-provided display-total share.

### RA-013 · MEDIUM · CONFIRMED BUG · Bulk import commits without confirmation
- `src/pages/ImportsPage.tsx:478` — `commitMutation.mutate()` on click; every other destructive action uses `window.confirm`. Mitigated by preview + dedupe, but N ledger rows are written. Fix: confirmation summarizing "import X rows into Account Y".

### RA-014 · MEDIUM · CONFIRMED BUG · Query failures render fake empty states
- `src/pages/InvestmentsPage.tsx:100-102/658-751` (trades, valuations, portfolio), `ImportsPage.tsx:106-109/544-553`, `BackupsPage.tsx:71-74/226-235`, `LiabilitiesPage.tsx:147-150/358-391`. On error the UI shows "No trades recorded", "No imports yet", "No database backup yet", or silently omits summary cards. Fix: render the error (pattern already used on TransactionsPage).

### RA-015 · MEDIUM · CONFIRMED BUG · Editing an account-level valuation silently re-assigns it
- `src/pages/InvestmentsPage.tsx:362-373` (`openEditValuation`): `instrumentId: valuation.instrumentId ?? instruments[0]?.id` — an account-level valuation is re-pointed to the first instrument on save. Fix: keep `null` and render the account-level option.

### RA-016 · MEDIUM · CONFIRMED BUG (schema) · Missing constraints
- `migrations/0001_init.sql:507-509` — `liability_payments.principal/interest/fee_minor` have no CHECK (negatives only rejected in service code); `:435` — `investment_trades.fee_minor` no CHECK; `:445-459` — no `UNIQUE (tenant_id, account_id, instrument_id, as_of_date)` on `investment_valuations`, so duplicate same-day valuations are storable and masked by `ORDER BY … LIMIT 1`. Fix: add CHECK/UNIQUE in a forward migration (dedupe first).

### RA-017 · MEDIUM · CONFIRMED BUG · Error path spins forever
- Pattern `query.isLoading || !data ? <Spinner/> : …` after an error alert: `IncomePage.tsx:337`, `AnalyticsPage.tsx:160`, `HealthPage.tsx:117`, `NetWorthPage.tsx:224-227`, `CalendarPage.tsx:92-95`. On error the spinner never resolves. Fix: include `isError` in the loading condition.

### RA-018 · MEDIUM · DESIGN WEAKNESS · Frontend performs money arithmetic
- `DashboardPage.tsx:40` (`assetsMinor + liabilitiesMinor`), `:134-139/298-327/406`, `NetWorthPage.tsx:28-31/57-68/320`, `AnalyticsPage.tsx:78/293`, `AccountsPage.tsx:488/536-539`, `FiPage.tsx:25-36`, `LabPage.tsx:244-253`, `IncomePage.tsx:176-182/419-420`. All are display proportions, and none currently produce a wrong *reported* total — but they contradict the stated architecture ("the interface never performs financial arithmetic") and already produced RA-012. Fix: backend-provided shares where the number is shown; keep JS scaling only for pure bar geometry.

### RA-019 · LOW · CONFIRMED BUG (UX) · "Paid" status warns but doesn't block in the UI
- `src/pages/LiabilitiesPage.tsx:854-858`. The backend enforces the rule (`liabilities.rs:582-585`, test `:1314`), so data is safe; the user just gets a post-submit error. Fix: block submit client-side.

### RA-020 · MEDIUM · DESIGN WEAKNESS · Missing FK/table indexes
- `0001_init.sql`: no index on `transactions.income_source_id` (:223), `counter_account_id` (:221), `fx_rate_id` (:231), `transaction_splits.category_id` (:279), `transaction_tags.tag_id` (:289), `liabilities.account_id` (:485), `investment_valuations.instrument_id` (:449), and several other FKs; several tenant-scoped tables (`budgets`, `goals`, `liabilities`, `income_sources`, `import_jobs`, `brokers`, `backups`) have no `tenant_id` index. Fix: forward migration adding indexes.

### RA-021 · MEDIUM · DESIGN WEAKNESS · N+1 queries across list endpoints
- Representative sites: `tx.rs:983-1033` (currency meta per transaction row), `accounts.rs:217-296` + `:128-139` (per-account and per-instrument queries), `income.rs:239-257/274`, `liabilities.rs:397-398`, `goals.rs:260/273`, `investments.rs:433/459/809/1061`, `networth.rs:84`, `api/mod.rs:74-82`, `api/accounts.rs:329-352`. Fix: batch currency meta (it is static per code) and join instead of per-row subqueries.

### RA-022 · LOW · CONFIRMED BUG · Keyring secret validated by length only
- `db.rs:77` accepts any 64-byte string; non-hex passes here and fails later in `open()` (`db.rs:15-16`) with "invalid database key format" and no recovery guidance. Fix: validate hex at read; error message should explain recovery.

### RA-023 · LOW · POTENTIAL BUG · XLSX zip-bomb
- `imports.rs:1073-1086` (calamine materializes the sheet). The 64 MiB cap applies to the compressed file only. Fix: cap decompressed rows/bytes, or parse sheet lazily with early stop at `MAX_ROWS`.

### RA-024 · LOW · CONFIRMED BUG (minor) · LIKE wildcards in search inputs
- `tx.rs:908`, `journal.rs:35` — `%`/`_` in search text alter LIKE semantics (bound parameters prevent injection; results only). Fix: escape wildcards.

### RA-025 · LOW · DESIGN WEAKNESS · Audit write after commit
- e.g. `tx.rs:1497-1506` and most mutators: audit failure returns `Err` although the financial write already committed, misleading the user. Fix: audit inside the same transaction.

### RA-026 · LOW · CONFIRMED BUG (a11y) · Accessible-name gaps
- 8 icon-only buttons: `InvestmentsPage.tsx:540-559`, `BudgetsPage.tsx:641-643`, `GoalsPage.tsx:465-481`, `LiabilitiesPage.tsx:581-597`, `BackupsPage.tsx:273-288`, `SettingsPage.tsx:456-458/572-574`. 3 unlabeled search inputs: `TransactionsPage.tsx:146`, `AccountsPage.tsx:198`, `JournalPage.tsx:132`. `th scope` missing on 9 tables (only `IncomePage.tsx:472-487` correct). `ui.tsx:150-226` modal has no focus trap/restore. `Toast.tsx:55-89` has no `aria-live`. Fix: aria-labels, scope, focus trap, live region.

### RA-027 · LOW · CONFIRMED BUG (UX) · Silent numeric coercion / weak client validation
- `BudgetsPage.tsx:216` (`Number(x) || 28` silently defaults), `AccountsPage.tsx:165-169` (NaN possible), `SettingsPage.tsx:249-316` (min/max not enforced), `LabPage.tsx:150/327/460` (`|| 0`), `FiPage.tsx:174` (NaN). Backend validates correctly, so errors appear late and cryptically. Fix: validate before submit (IncomePage's pay-day check at `:252-258` is the model).

### RA-028 · LOW · CONFIRMED BUG (UX) · "Export verified" title shown on failure
- `BackupsPage.tsx:426` — `title="Export verified"` constant even when `verifySummary.valid === false`. Fix: derive title from `valid`.

### RA-029 · LOW · CONFIRMED BUG (dead UI) · Unreachable health empty state
- `HealthPage.tsx:252-258` — `reportQuery.isSuccess && !report` can never be true. Fix: condition on data presence differently or remove.

### RA-030 · LOW · CONFIRMED BUG (display) · UTC timestamps rendered as local dates
- `JournalPage.tsx:205`, `ImportsPage.tsx:590`, `BackupsPage.tsx:260/430/450`, `TransactionDetail.tsx:220-222`, `SettingsPage.tsx:603` — `slice(0,10)`/string-replace on ISO timestamps can show a different day than `formatDate` on local dates. Fix: one timestamp formatter.

### RA-031 · LOW · MISSING FUNCTIONALITY surfaced in UI · Budget rollover inert
- `BudgetsPage.tsx:477-482` admits carry-over is not folded into totals; `budgets.rs` stores `rollover_policy` but never applies it. Settings' "Scheduled backups" toggle (`SettingsPage.tsx:289-304`) likewise enables nothing. Both are honestly labeled; still, controls that do nothing should be disabled until implemented.

### RA-032 · LOW · OPTIONAL · Hardcoded calculator defaults
- `FiPage.tsx:95-99` ("4", "6", "2.5"), `LabPage.tsx:100-101/290/417`, `BudgetsPage.tsx:262/351` ("28→27" text). Keep in sync with backend defaults or read from settings.

### RA-033 · LOW · OPTIONAL · Dead code and unused dependencies
- Unused deps: `base64`, `regex`, `serial_test` (`Cargo.toml:43/47/53`). Dead `pub` fns: `accounts.rs` `account_balance_in_base`, `networth.rs` `latest_base_currency`. Unused schema: `import_rules` (incl. an unimplemented `'regex'` match type, `0001_init.sql:607`), `recurring_rules`/`recurring_occurrences` (part of RA-040). `Shell.tsx:189-193` "soon" badge dead. `HealthPage.tsx:252` (RA-029).

### RA-034 · INFORMATIONAL · Error strings echo input and raw SQL errors
- `money.rs:101-134`, `dates.rs:15`, `imports.rs` issue strings (persisted to `import_rows.error_message`), `db.rs:34-36`/`backups.rs:149-151` (raw rusqlite text). Acceptable for a local app; worth normalizing before any crash-reporting exists.

### RA-035 · INFORMATIONAL · Dynamic SQL via `format!`
- `api/mod.rs:202-205` (`count_table`) interpolates the table name — call sites pass literals only. Keep an enum-typed parameter if this ever grows.

---

## 14. Missing Features and Incomplete Workflows

1. **Recurring transactions** — schema only (`0001_init.sql:154-187`); no scheduler, no UI. (README discloses.)
2. **Reconciliation worksheet** — per-transaction "reconciled" status only. (Disclosed.)
3. **Scheduled backups** — preference stored, never executed. (Disclosed.)
4. **Income-source backfill** — existing transactions cannot be linked to a source. (Disclosed.)
5. **Bills & subscriptions** — absent as a concept (partial substitutes: liability due days).
6. **Notifications/reminders** — absent.
7. **CSV/report export** — only full encrypted JSON dumps; displayed reports cannot be exported.
8. **Exchange-rate deletion/correction** — rates are immutable with no delete (good for auditability; user can only add).
9. **App lock / auto-update** — placeholder labels in Settings (`SettingsPage.tsx:333/345`).
10. **Goal/liability history preservation** on delete (RA-004).

---

## 15. Recommended Improvements

Beyond the issue fixes: centralize the percent/rounding helpers in `money.rs`; add a shared `<QueryState>` component for loading/error/empty (fixes RA-014/017 uniformly); move displayed percentages to backend payloads (RA-018); standardize timestamps (RA-030); batch currency metadata (RA-021); add `th scope`/aria sweep (RA-026); introduce component tests for the pages that render computed numbers; document the cost-basis methodology (average cost — `investments.rs:424-445`) in the README.

---

## 16. Prioritized Action Plan

**PHASE 1 — Critical financial correctness and data integrity** (no dependencies; do first)
1. RA-001 (draft edit data loss) → 2. RA-002 (liability payment atomicity) → 3. RA-005 (checked sums) → 4. RA-006/RA-007 (remaining non-atomic writes) → 5. RA-016 (schema CHECK/UNIQUE, needs a forward migration + dedupe of existing valuation rows) → 6. RA-004 decision (soft-delete vs. explicit destructive gate).

**PHASE 2 — Security and reliability** (independent of Phase 1)
1. RA-003/RA-022 (keyring handling) → 2. RA-009 (audit redaction/allow-list) → 3. RA-010 (path restrictions, size caps) → 4. RA-023 (zip-bomb) → 5. RA-025 (audit-in-transaction).

**PHASE 3 — Broken features and incomplete workflows** (after Phase 1 where they touch writes)
1. RA-008 (import commit resilience) → 2. RA-013 (commit confirmation) → 3. RA-011 (count semantics) → 4. RA-012/RA-018 (numbers displayed consistently) → 5. RA-015 (valuation edit) → 6. RA-028/RA-029 → 7. RA-031 (disable or implement rollover/scheduling).

**PHASE 4 — UX and accessibility** (parallel with Phase 3)
RA-026 (names/labels/focus), RA-017/RA-014 (query states), RA-030 (timestamps), RA-027 (client validation), RA-019.

**PHASE 5 — Performance and maintainability** (before data volumes grow)
RA-020 (indexes migration), RA-021 (batch meta/joins), RA-033 (dead code/deps), code-splitting, shared percent helpers.

**PHASE 6 — Optional enhancements**
CSV/report export, recurring execution, reconciliation worksheet, scheduled backups, bills/subscriptions, notifications, RA-032 polish.

---

## 17. Test Execution Results

All commands executed on this machine during the audit (commit `7d2b039`):

| Command | Result | Evidence |
|---|---|---|
| `scripts\cargo-env.bat test` | **119 passed, 0 failed** (3 suites) | `%TEMP%\opencode\final-cargo-test.txt` |
| `scripts\cargo-env.bat fmt -- --check` | **clean** (0 diffs) | `final-fmt.txt` |
| `scripts\cargo-env.bat clippy --all-targets` | **no errors**; 15 warnings (10 dead-code: `period_range`, `account_balance_in_base`, `format_scaled`, `latest_portfolio_value`, `latest_base_currency`, unused fields, `Date` variant; 5 style: identical if-blocks, too-many-args, complex type, unused dep `base64`) | `final-clippy.txt` |
| `npm test` | **10 passed** (display helpers only) | `final-vitest.txt` |
| `npm run typecheck` | **clean** | `final-tsc.txt` |
| `npm run build` | **clean** (Vite >500 kB chunk warning) | `final-build.txt` |

Not executed (not possible in this environment): E2E/UI tests (none exist), Windows installer signing check (no certificate), runtime performance profiling.

## 18. Final Assessment

Rawjan's financial core — the part users must trust — is sound: strict parsing, integer minor units, double-entry invariants, reversal semantics, historical FX snapshots, and honest reporting all hold up under trace-level inspection and are protected by a meaningful test suite. The five previously raised calculation defects are verifiably fixed.

What prevents an unqualified "release-ready" verdict is durability and display honesty around the edges: one edit path can destroy a record (RA-001), payment writes can split-brain (RA-002), a corrupted credential entry can lock a user out of their own vault (RA-003), and a handful of screens show numbers or states that don't match what the backend computed (RA-011/012/014). All are fixable without architectural change. Clear Phase 1 and Phase 2, add the missing regression tests from §11, and this becomes a product that deserves the trust its design is aiming for.
