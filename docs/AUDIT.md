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
- **Local-first:** no telemetry, no cloud calls, no bank scraping, no runtime network requests.

## Fixed in this round (Phase 1)

| Issue | Before | After |
| --- | --- | --- |
| Lenient amount parsing | `12abc34` became `1234` because invalid characters were filtered out | Strict grammar: optional leading sign, digits, grouping commas only in threes, one decimal point; anything else is rejected. Shared by amounts, FX rates, FI/lab percentages, investment quantities/prices, liability rates |
| Overflow in `amount_to_minor` | Unchecked decimal scale/multiply | `checked_pow` + `checked_mul`, out-of-range values return a validation error |
| Overflow in `convert_minor` | Saturated silently to `i64::MIN`/`MAX`, corrupting balances | Returns `AppResult`, errors propagate through `create_transaction`, `convert_to_currency` and split allocation |
| Import amount parsing | Filtered characters anywhere in the cell | Only the digit span is parsed; currency codes, symbols and signs are decoration, embedded garbage such as `1x2.50` is rejected and surfaces as a row issue |
| Dead code | `services::accounts::base_minor` wrapper | Removed with the signature change |

Verification: 94 Rust tests, 10 Vitest tests, strict `tsc --noEmit`, production build.

## Known risks and debt

### Financial integrity
- FI/lab projections still multiply decimals over long loops; extreme inputs can overflow a decimal
  and panic instead of returning a validation error. Needs checked arithmetic in projection loops.
- Split allocation assigns the rounding remainder to the last leg; this is documented but should get
  property-style tests over random splits and rates.
- No automated invariant check that every stored journal balances (`SUM(debit) = SUM(credit)`) on an
  existing vault; invariants are enforced at write time and covered by unit tests only.

### Data protection
- Restore is a documented manual file swap. There is no guided, verified restore flow.
- Scheduled backup preference is stored but nothing executes it; no retention pruning.
- `RAWJAN_DB_KEY` bypasses the credential store; documented as less secure, must stay out of logs.
- Installer is unsigned, so SmartScreen warns. No update channel.

### Product completeness
- Reconciliation is a per-transaction status; no statement-matching worksheet.
- Recurring rules exist in the schema but nothing schedules or posts them.
- Existing transactions cannot be linked to an income source after creation; only new entries can.
  Expected-versus-received therefore under-reports for historical income until backfill exists.
- No component or end-to-end tests; frontend coverage is helper-level only.
- Income estimates deliberately exclude weekly/biweekly/irregular sources and cross-currency sources.
- The main JS bundle is ~609 kB before gzip; pages are not code-split.

### Engineering quality
- No CI pipeline yet (rustfmt, clippy, tests, typecheck, frontend tests, build).
- The Rust tree is not `rustfmt`-clean; formatting needs one dedicated commit before CI can enforce it.
- `base64` is declared but unused; the linker emits OpenSSL PDB warnings on MSVC builds (cosmetic).
- Modal focus management and keyboard traversal are basic; accessibility has not been audited.

## Prioritised plan

1. **Financial integrity** — strict parsing and checked overflow (done here); checked projection
   arithmetic; split/reversal property tests; vault balance invariant check.
2. **Data protection** — scheduled backups with retention, verified guided restore, export
   verification UX, key-handling review.
3. **Product completeness** — income-source backfill/relink, reconciliation worksheet, recurring
   execution, reporting exports.
4. **UI/UX** — accessibility pass (focus, labels, contrast), responsive behaviour, code-splitting.
5. **Engineering quality** — rustfmt commit, clippy, GitHub Actions CI, component and E2E tests.
6. **Release readiness** — signing, update channel, crash-free launch verification on a clean machine.
