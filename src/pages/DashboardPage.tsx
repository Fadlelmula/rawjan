import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import clsx from "clsx";
import type { ReactNode } from "react";
import {
  ArrowDownLeft,
  ArrowLeftRight,
  ArrowRight,
  ArrowUpRight,
  ChevronRight,
  CircleAlert,
  PiggyBank,
  Plus,
  Receipt,
  Sparkles,
  Target,
  Wallet,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import {
  amountTone,
  formatDate,
  money,
  percentOf,
  signedAmount,
} from "@/lib/display";
import { useComposer } from "@/components/ComposerContext";
import { Alert, Badge, EmptyState, Panel, SectionLabel } from "@/components/ui";

export function DashboardPage() {
  const { app, netWorth, accounts, counts, currencies } = useBootstrap();
  const { openComposer } = useComposer();

  const recent = useQuery({
    queryKey: ["transactions", "recent"],
    queryFn: () => api.listTransactions({ limit: 6, status: "effective" }),
  });

  const assetShare = percentOf(netWorth.assetsMinor, netWorth.assetsMinor + netWorth.liabilitiesMinor);
  const hasAccounts = accounts.length > 0;
  const hasActivity = (recent.data?.total ?? 0) > 0;
  const primary = currencies.find((currency) => currency.code === app.primaryCurrency);

  return (
    <div className="space-y-5 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <SectionLabel>{app.primaryCurrency} · {primary?.name ?? "Primary currency"}</SectionLabel>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">
            Your ledger at a glance
          </h2>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => openComposer("income")}
            className="inline-flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.025] px-2.5 py-1.5 text-[0.7rem] text-zinc-400 transition hover:border-white/20 hover:text-zinc-100"
          >
            <ArrowDownLeft className="h-3.5 w-3.5 text-sage-400" />
            Income
          </button>
          <button
            type="button"
            onClick={() => openComposer("transfer")}
            className="inline-flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.025] px-2.5 py-1.5 text-[0.7rem] text-zinc-400 transition hover:border-white/20 hover:text-zinc-100"
          >
            <ArrowLeftRight className="h-3.5 w-3.5 text-champagne-400" />
            Transfer
          </button>
        </div>
      </div>

      {netWorth.missingRates.length > 0 ? (
        <Alert
          tone="warning"
          title="Some balances are excluded"
          action={
            <Link to="/settings" className="shrink-0 text-xs text-amber-200 underline underline-offset-4">
              Add rates
            </Link>
          }
        >
          No exchange rate is stored for {netWorth.missingRates.join(", ")}. Accounts in those currencies are
          left out of net worth rather than converted with a guessed value.
        </Alert>
      ) : null}

      <div className="grid gap-4 xl:grid-cols-[1.55fr_1fr]">
        <Panel className="relative overflow-hidden p-5 sm:p-6">
          <div
            className="pointer-events-none absolute -right-24 -top-28 h-64 w-64 rounded-full opacity-60 blur-3xl"
            style={{ background: "radial-gradient(circle, rgba(179,36,58,0.35), transparent 68%)" }}
          />
          <div className="relative">
            <div className="flex items-start justify-between gap-4">
              <div>
                <SectionLabel>Net worth</SectionLabel>
                <div className="mt-2 flex flex-wrap items-baseline gap-2">
                  <span className="font-display text-[2.4rem] leading-none tracking-[-0.01em] text-zinc-50">
                    {netWorth.net}
                  </span>
                  <span className="text-[0.68rem] uppercase tracking-[0.16em] text-zinc-500">
                    {netWorth.currency}
                  </span>
                </div>
                <div className="mt-2 text-[0.68rem] text-zinc-600">
                  As of {formatDate(netWorth.asOfDate)} · {netWorth.accountsIncluded} active account
                  {netWorth.accountsIncluded === 1 ? "" : "s"} included
                </div>
              </div>
              <Badge tone={netWorth.netMinor >= 0 ? "green" : "red"}>
                {netWorth.netMinor >= 0 ? "Positive position" : "Negative position"}
              </Badge>
            </div>

            <div className="mt-6 grid grid-cols-2 gap-3">
              <div className="rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-3">
                <div className="text-[0.58rem] uppercase tracking-[0.17em] text-zinc-600">Assets</div>
                <div className="mt-1 font-display text-lg text-zinc-100">
                  {money(netWorth.assets, netWorth.currency)}
                </div>
              </div>
              <div className="rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-3">
                <div className="text-[0.58rem] uppercase tracking-[0.17em] text-zinc-600">Liabilities</div>
                <div className="mt-1 font-display text-lg text-zinc-100">
                  {money(netWorth.liabilities, netWorth.currency)}
                </div>
              </div>
            </div>

            <div className="mt-4">
              <div className="flex h-1.5 overflow-hidden rounded-full bg-white/[0.05]">
                <div className="bg-lily-500" style={{ width: `${assetShare}%` }} />
                <div className="bg-champagne-500/70" style={{ width: `${100 - assetShare}%` }} />
              </div>
              <div className="mt-2 flex items-center justify-between text-[0.62rem] text-zinc-600">
                <span>Asset share</span>
                <span>{Math.round(assetShare)}% / {Math.round(100 - assetShare)}%</span>
              </div>
            </div>

            <div className="mt-5 grid gap-2 sm:grid-cols-2">
              <div className="flex items-center gap-2.5 rounded-lg border border-white/[0.05] bg-white/[0.015] px-3 py-2">
                <Wallet className="h-3.5 w-3.5 text-sage-400" />
                <div className="min-w-0">
                  <div className="text-[0.56rem] uppercase tracking-[0.16em] text-zinc-600">Liquid cash</div>
                  <div className="truncate text-[0.76rem] text-zinc-300">
                    {money(netWorth.cash, netWorth.currency)}
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-2.5 rounded-lg border border-white/[0.05] bg-white/[0.015] px-3 py-2">
                <Sparkles className="h-3.5 w-3.5 text-champagne-400" />
                <div className="min-w-0">
                  <div className="text-[0.56rem] uppercase tracking-[0.16em] text-zinc-600">Invested</div>
                  <div className="truncate text-[0.76rem] text-zinc-300">
                    {money(netWorth.invested, netWorth.currency)}
                  </div>
                </div>
              </div>
            </div>

            {!hasAccounts ? (
              <div className="mt-5 flex flex-wrap items-center gap-3 rounded-xl border border-lily-500/25 bg-lily-600/[0.08] px-4 py-3">
                <div className="min-w-0 flex-1 text-xs text-lily-100/90">
                  Start by creating your first account. No financial figures are seeded or invented.
                </div>
                <Link to="/accounts">
                  <span className="inline-flex items-center gap-1 rounded-lg border border-lily-400/40 bg-lily-600/20 px-3 py-1.5 text-xs text-white">
                    Add account <ArrowRight className="h-3.5 w-3.5" />
                  </span>
                </Link>
              </div>
            ) : null}
          </div>
        </Panel>

        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4 xl:grid-cols-2">
          <StatCard
            label="Transactions"
            value={String(counts.transactions)}
            hint="All statuses"
            icon={<Receipt className="h-4 w-4" />}
            to="/transactions"
          />
          <StatCard
            label="Accounts"
            value={String(counts.accounts)}
            hint="Active"
            icon={<Wallet className="h-4 w-4" />}
            to="/accounts"
          />
          <StatCard
            label="Budgets"
            value={String(counts.budgets)}
            hint={counts.budgets === 0 ? "Not configured" : "Active"}
            icon={<Target className="h-4 w-4" />}
            to="/budgets"
          />
          <StatCard
            label="Savings goals"
            value={String(counts.goals)}
            hint={counts.goals === 0 ? "None yet" : "In progress"}
            icon={<PiggyBank className="h-4 w-4" />}
            to="/savings"
          />
        </div>
      </div>

      <div className="grid gap-4 xl:grid-cols-[1.55fr_1fr]">
        <Panel className="overflow-hidden">
          <div className="flex items-center justify-between gap-3 border-b border-white/[0.055] px-5 py-3.5">
            <div>
              <SectionLabel>Recent activity</SectionLabel>
              <div className="mt-0.5 text-[0.68rem] text-zinc-600">Posted and reconciled entries only</div>
            </div>
            <Link
              to="/transactions"
              className="inline-flex items-center gap-1 text-[0.68rem] text-zinc-500 transition hover:text-zinc-200"
            >
              View ledger <ChevronRight className="h-3 w-3" />
            </Link>
          </div>

          {recent.isLoading ? (
            <div className="space-y-2 p-4">
              {[0, 1, 2, 3].map((index) => (
                <div key={index} className="h-12 animate-pulse rounded-lg bg-white/[0.03]" />
              ))}
            </div>
          ) : recent.isError ? (
            <div className="p-5">
              <Alert tone="error" title="Could not load activity">
                {errorMessage(recent.error)}
              </Alert>
            </div>
          ) : !hasActivity ? (
            <div className="p-5">
              <EmptyState
                icon={<Receipt className="h-5 w-5" />}
                title="The ledger is quiet"
                description="Record an expense, income or transfer to begin. Every entry creates balanced double-entry legs."
                action={
                  <button
                    type="button"
                    onClick={() => openComposer("expense")}
                    className="inline-flex items-center gap-1.5 rounded-lg bg-lily-500 px-3 py-1.5 text-xs font-medium text-white transition hover:bg-lily-400"
                  >
                    <Plus className="h-3.5 w-3.5" /> Record first entry
                  </button>
                }
              />
            </div>
          ) : (
            <ul className="divide-y divide-white/[0.045]">
              {recent.data?.rows.map((row) => (
                <li key={row.id}>
                  <Link
                    to="/transactions"
                    className="rawjan-table-row flex items-center gap-3 px-5 py-3"
                  >
                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-white/[0.07] bg-white/[0.02] text-zinc-500">
                      {row.kind === "income" ? (
                        <ArrowDownLeft className="h-3.5 w-3.5 text-sage-400" />
                      ) : row.kind === "expense" || row.kind === "fee" ? (
                        <ArrowUpRight className="h-3.5 w-3.5 text-lily-300" />
                      ) : (
                        <ArrowLeftRight className="h-3.5 w-3.5 text-champagne-400" />
                      )}
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[0.8rem] text-zinc-200">{row.description}</div>
                      <div className="mt-0.5 truncate text-[0.63rem] text-zinc-600">
                        {formatDate(row.localDate)} · {row.accountName}
                        {row.categoryName ? ` · ${row.categoryName}` : ""}
                      </div>
                    </div>
                    <div className="text-right">
                      <div className={clsx("font-mono text-[0.78rem]", amountTone(row.kind))}>
                        {signedAmount(row.kind, row.amount)}
                      </div>
                      <div className="mt-0.5 text-[0.58rem] uppercase tracking-wider text-zinc-600">
                        {row.accountCurrency}
                      </div>
                    </div>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <div className="space-y-4">
          <Panel className="p-5">
            <SectionLabel>Allocation</SectionLabel>
            <div className="mt-3 space-y-3">
              <AllocationRow
                label="Liquid cash"
                minor={netWorth.cashMinor}
                total={netWorth.assetsMinor}
                tone="bg-sage-500/80"
                display={money(netWorth.cash, netWorth.currency)}
              />
              <AllocationRow
                label="Investments"
                minor={netWorth.investedMinor}
                total={netWorth.assetsMinor}
                tone="bg-lily-500/80"
                display={money(netWorth.invested, netWorth.currency)}
              />
              {netWorth.receivablesMinor > 0 ? (
                <AllocationRow
                  label="Money lent"
                  minor={netWorth.receivablesMinor}
                  total={netWorth.assetsMinor}
                  tone="bg-champagne-300/70"
                  display={money(netWorth.receivables, netWorth.currency)}
                />
              ) : null}
              <AllocationRow
                label="Other assets"
                minor={netWorth.otherAssetsMinor}
                total={netWorth.assetsMinor}
                tone="bg-champagne-500/70"
                display={money(netWorth.otherAssets, netWorth.currency)}
              />
            </div>
            <div className="mt-4 border-t border-white/[0.05] pt-3 text-[0.63rem] leading-relaxed text-zinc-600">
              Allocation uses recorded balances. Investment accounts use the latest manual valuation when one
              exists, otherwise the ledger balance.
            </div>
          </Panel>

          <Panel className="p-5">
            <div className="flex items-center justify-between">
              <SectionLabel>Module status</SectionLabel>
              <Badge tone="champagne">Phase 1</Badge>
            </div>
            <div className="mt-3 space-y-2 text-[0.72rem]">
              <StatusLine label="Ledger & accounts" state="Live" live />
              <StatusLine label="Budgets & goals" state="Live" live />
              <StatusLine label="Investments & liabilities" state="Live" live />
              <StatusLine label="Net worth history" state="Live" live />
              <StatusLine label="Imports & backups" state="Planned" />
            </div>
            <div className="mt-3 text-[0.63rem] leading-relaxed text-zinc-600">
              Planned modules are visibly marked throughout the application. Nothing is presented as finished
              before it is.
            </div>
          </Panel>

          {counts.transactions === 0 ? (
            <div className="flex items-center gap-2 rounded-xl border border-white/[0.05] bg-white/[0.015] px-4 py-3 text-[0.65rem] text-zinc-600">
              <CircleAlert className="h-3.5 w-3.5 shrink-0 text-champagne-500" />
              Tip: pending entries affect balances but stay out of income and expense reports until posted.
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function StatCard({
  label,
  value,
  hint,
  icon,
  to,
}: {
  label: string;
  value: string;
  hint: string;
  icon: ReactNode;
  to: string;
}) {
  return (
    <Link to={to} className="group">
      <Panel className="h-full p-4 transition-colors group-hover:border-white/15">
        <div className="flex items-start justify-between gap-2">
          <div className="text-[0.58rem] font-semibold uppercase tracking-[0.16em] text-zinc-600">{label}</div>
          <span className="text-zinc-600 transition group-hover:text-lily-300">{icon}</span>
        </div>
        <div className="mt-3 font-display text-2xl leading-none text-zinc-100">{value}</div>
        <div className="mt-1 text-[0.6rem] text-zinc-600">{hint}</div>
      </Panel>
    </Link>
  );
}

function AllocationRow({
  label,
  minor,
  total,
  tone,
  display,
}: {
  label: string;
  minor: number;
  total: number;
  tone: string;
  display: string;
}) {
  const share = percentOf(minor, total);
  return (
    <div>
      <div className="flex items-center justify-between text-[0.68rem]">
        <span className="text-zinc-400">{label}</span>
        <span className="font-mono text-[0.66rem] text-zinc-500">{display}</span>
      </div>
      <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-white/[0.05]">
        <div className={clsx("h-full rounded-full", tone)} style={{ width: `${share}%` }} />
      </div>
    </div>
  );
}

function StatusLine({ label, state, live = false }: { label: string; state: string; live?: boolean }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-zinc-400">{label}</span>
      <span className={clsx("inline-flex items-center gap-1.5 text-[0.6rem]", live ? "text-sage-400" : "text-zinc-600")}>
        <span className={clsx("h-1 w-1 rounded-full", live ? "bg-sage-400" : "bg-zinc-700")} />
        {state}
      </span>
    </div>
  );
}
