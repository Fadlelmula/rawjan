# Rawjan engineering audit — 2026-11-16

Scope: architecture, financial integrity, data protection, product completeness, UX, engineering
quality and release readiness. This is a working document; items move to "done" only when code and
tests exist.

## Architecture

- **Shell:** Tauri 2, Windows/WebView2, per-user NSIS installer. React + TypeScript + Tailwind
  frontend, hash routing, TanStack Query for server state.
- **Backend:** thin Tauri commands in `src-tauri/src/api/` call services in
  `src-tauri/src/services/`; SQL lives in the services. `state.rs` owns the single SQLCipher
  connection and the tenant/user boundary.
- **Money:** integer minor units in SQLite (`i64`), `rust_decimal` for rates and scaling. The
  frontend only renders backend-formatted `amount`/`baseAmount` strings; it never does monetary
  arithmetic.
- **Ledger:** real double-entry `ledger_entries` per transaction; posted records are immutable and
  corrected by reversal, drafts are the only deletable rows. Original currency, original amount,
  historical FX snapshot and base amount are stored per transaction.
- **Storage:** SQLCipher-encrypted SQLite, WAL mode, foreign keys on. Key in Windows Credential
  Manager (`rawjan-finance`), 64-hex `RAWJAN_DB_KEY` override for testing/portable use.
- **Local-first:** no telemetry, no cloud calls, no bank connections, no runtime network requests.

## Fixed in the financial-correctness round

| Issue | Before | After |
| --- | --- | --- |
| Income entry counts | "Income entries" counted every effective row, including expenses, transfers and openings | Income/dividend, expense/fee/refund and general activity are counted separately; the income report headline uses income rows only (`incomeCount`, `expenseCount`, `generalCount`, `transactionCount` on each month) |
| Category percentages | Percentages were shares of the displayed top rows only | The complete eligible total is computed before the 12-row display limit; with 13 equal categories each row shows 7%, not 8% |
| Budget FX failure | Unconverted base-currency spend was silently substituted into a budget-currency total | Spend is `null` with `fxUnavailable` on the allocation and budget; the UI shows an explicit notice and totals exclude the amount |
| Semimonthly income | Two payouts were counted for every touched month, even one-day ranges | Payouts are placed at the pay day and half a month later (clamped to month end), each checked against the range; partial months count only what they contain; sources without a pay day get no estimate |
| One-time income | Any receipt at any date, including future ones, removed the expectation from historical ranges | Only receipts dated on or before the range end suppress the one-time expectation |
| Investment account valuation (net worth) | The single latest valuation row stood in for the whole account, so accounts with several holdings reported one holding's value | The account value sums each holding's latest valuation on or before the date; an unvalued holding falls back to the ledger cost basis instead of a partial sum |
| Linked money-lent double count | A money-lent receivable linked to an account was counted in both the account balance and the receivables | Any account linked to an active liability record is counted from that record only |
| Liability payment edits | Editing a payment left the status stale (`paid` with a reopened balance) and rejected legitimate principal re-use | Editing recomputes outstanding and syncs `paid`/`active`; the principal limit accounts for the payment being edited |
| Partial market values | Instruments and portfolio summaries displayed partial market sums as if complete | A market value is reported only when every holding has a valuation; `valuedHoldings` communicates partial state |
| Portfolio FX failures | Unconvertible currencies were silently dropped from the total | `latest_portfolio_value` returns the total plus the currencies it could not convert |
| Overflow in totals | Summing extreme amounts could wrap around in release builds | `checked_sum`/`checked_diff` in `money.rs` are used for report and budget totals; overflow is a validation error (regression test with two i64::MAX allocations) |

## Fixed earlier (parsing and overflow)

- Strict decimal grammar for amounts, FX rates, percentages, investment quantities/prices and
  liability rates; invalid characters are rejected, never stripped.
- Checked pow/mul in `amount_to_minor` and `convert_minor`; overflow returns a validation error
  instead of saturating to `i64::MIN`/`i64::MAX`.
- Import amounts parse only the digit span: currency decoration is allowed, embedded garbage is
  rejected and surfaces as a row issue.

## Fixed in the hardening round (write atomicity, integrity, display honesty)

| Issue | Fix |
| --- | --- |
| Draft/pending edits deleted then re-created non-atomically (data loss on failure) | `update_transaction` runs delete + re-create in one transaction (`tx.rs`) |
| Liability payment save/delete split-brain risk | `save_payment`/`delete_payment` wrap payment rows and balance updates in one transaction; failure leaves no partial state (regression test with an injected trigger failure) |
| Posted-path category edits could desynchronize `transactions`/`ledger_entries` | the metadata + category edit now commits as one transaction |
| Account creation with an opening balance could leave an account without its ledger entry | account insert + opening transaction + audit commit together |
| Import commit could leave rows imported while the job read "parsed" | the whole commit runs in one transaction with per-row savepoints |
| Release-build `i64` wrap could bypass split/payment "must add up" checks | `checked_sum`/`checked_diff` applied to splits, leg allocation, payment components and net-worth totals |
| Deleting a goal/liability cascade-erased contributions/payments | deletion is refused while recorded history exists (archive instead); UI confirm text matches |
| Silent key rotation when the Credential Manager entry is malformed | malformed keys now error with recovery guidance; a key is generated only when none exists |
| Settings accepted arbitrary keys; full values (and journal bodies) were duplicated into the audit trail | settings are a closed allow-list; `app_lock` values never audited; journal audits store metadata only |
| Arbitrary-path file reads | imports only read statement extensions; backup validation only opens `.db`-shaped files; export verification checks extension and caps the read at 256 MB |
| Duplicate daily valuations were silently masked | unique index on (account, instrument, day); older duplicates are removed in the migration |
| Account-level valuations were silently re-pointed to the first instrument when edited | `ValuationInput.instrument_id` is now optional end-to-end; the UI offers "Account-level" |
| Missing FK/tenant indexes | migration `0002_integrity_indexes.sql` adds 22 indexes |
| LIKE-wildcard injection in search | wildcards are escaped (`db::like_contains`, `ESCAPE '!'`) |
| Dashboard count hints claimed "Active"/"In progress" for unfiltered counts | hints now describe what is counted |
| Analytics bar widths disagreed with the printed percentages | bars follow the Rust-computed percent of the full total |
| Money percentages computed in JavaScript (Dashboard, Net worth, Accounts) | percentages moved into Rust (`NetWorth` shares, `AccountDto.credit_used_percent`); JS keeps only chart geometry |
| Query failures rendered fake empty states (Investments, Imports, Backups, Liabilities) and endless spinners (5 pages) | error branches added; spinner conditions no longer mask errors |
| Import commit had no confirmation | confirmation summarizing rows and scope |
| "Export verified" title shown even on failure | title reflects the actual result |
| Accessibility gaps | aria-labels on icon buttons and search inputs, `th scope` on all tables, modal focus trap + focus restore, toast `aria-live` region |
| Timestamps rendered as UTC slices that could show the wrong day | shared `formatDateTime` renders local time consistently |
| Inert controls (scheduled backups, budget rollover) looked functional | disabled with explicit "not implemented" hints |
| Client-side numeric coercion (NaN days, silent `|| 28`) | validation added on Accounts, Budgets, Liabilities, Settings, FI inputs |
| Unused dependencies and dead code | `base64`, `regex`, `serial_test` removed; three dead `pub` functions removed |

Verification after the round: 126 Rust tests (including a v1→v2 migration test with real rows),
10 Vitest tests, strict `tsc --noEmit`, production build, `cargo fmt --check`, `cargo clippy
--all-targets` without errors.

## Known risks and debt

### Financial integrity
- FI/lab projection loops still multiply decimals over long horizons; extreme inputs can overflow a
  decimal and panic instead of returning a validation error.
- No automated invariant check that an existing vault's journals all balance (`SUM(debit) =
  SUM(credit)`); invariants are enforced at write time and covered by unit tests.
- Split allocation assigns the rounding remainder to the last leg (documented); property-style tests
  over random splits and rates are still thin.

### Data protection
- Restore is a documented manual file swap. There is no guided, verified restore flow.
- Scheduled backup preference is stored but nothing executes it; no retention pruning.
- `RAWJAN_DB_KEY` bypasses the credential store; documented as less secure, must stay out of logs.
- Installer is unsigned, so SmartScreen warns. No update channel.

### Product completeness
- Reconciliation is a per-transaction status; no statement-matching worksheet.
- Recurring rules exist in the schema but nothing schedules or posts them.
- Existing transactions cannot be linked to an income source after creation; only new entries can.
- No component or end-to-end tests; frontend coverage is helper-level only.
- The main JS bundle is ~609 kB before gzip; pages are not code-split.

### Engineering quality
- No CI pipeline yet (rustfmt, clippy, tests, typecheck, frontend tests, build).
- `cargo fmt` is clean tree-wide; `cargo clippy --all-targets` still reports pre-existing
  dead-code warnings (`period_range`, `format_scaled`, unused struct fields) and a few style lints
  (complex types, too many arguments).
- `base64` is no longer a dependency (removed); the linker still emits OpenSSL PDB warnings on MSVC
  builds (cosmetic).
- Modal focus management now includes a focus trap and focus restoration; a full accessibility audit
  (screen-reader testing, contrast measurement) has not been performed.

## Prioritised plan

1. **Financial integrity** — strict parsing and checked overflow (done); separated counting,
   honest category percentages, FX-safe budgets and net worth (done); checked projection
   arithmetic and a vault balance invariant check (next).
2. **Data protection** — scheduled backups with retention, verified guided restore, export
   verification UX, key-handling review.
3. **Product completeness** — income-source backfill/relink, reconciliation worksheet, recurring
   execution, reporting exports.
4. **UI/UX** — accessibility pass (focus, labels, contrast), responsive behaviour, code-splitting.
5. **Engineering quality** — CI with fmt/clippy/tests/typecheck/build, component and E2E tests,
   resolve remaining clippy dead code.
6. **Release readiness** — signing, update channel, crash-free launch verification on a clean machine.
