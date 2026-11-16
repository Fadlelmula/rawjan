import { useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import clsx from "clsx";
import { ArrowDownRight, ArrowUpRight, ChartColumn, Users } from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { formatDate, money, percentOf } from "@/lib/display";
import { Alert, Badge, Button, EmptyState, Panel, SectionLabel, Spinner } from "@/components/ui";

function firstDayOfMonth(date: Date, offset: number): string {
  const shifted = new Date(date.getFullYear(), date.getMonth() + offset, 1);
  const month = String(shifted.getMonth() + 1).padStart(2, "0");
  return `${shifted.getFullYear()}-${month}-01`;
}

function todayInput(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

interface RangeOption {
  id: string;
  label: string;
  build: () => { from: string; to: string };
}

const RANGES: RangeOption[] = [
  {
    id: "3m",
    label: "3 months",
    build: () => ({ from: firstDayOfMonth(new Date(), -2), to: todayInput() }),
  },
  {
    id: "6m",
    label: "6 months",
    build: () => ({ from: firstDayOfMonth(new Date(), -5), to: todayInput() }),
  },
  {
    id: "12m",
    label: "12 months",
    build: () => ({ from: firstDayOfMonth(new Date(), -11), to: todayInput() }),
  },
  {
    id: "ytd",
    label: "Year to date",
    build: () => ({
      from: `${new Date().getFullYear()}-01-01`,
      to: todayInput(),
    }),
  },
];

export function AnalyticsPage() {
  const { app } = useBootstrap();
  const [rangeId, setRangeId] = useState("6m");
  const [from, setFrom] = useState(() => RANGES[1].build().from);
  const [to, setTo] = useState(() => RANGES[1].build().to);
  const [breakdown, setBreakdown] = useState<"expense" | "income">("expense");

  const reportQuery = useQuery({
    queryKey: ["analytics", from, to],
    queryFn: () => api.analyticsReport(from, to),
  });

  const report = reportQuery.data;
  const maxMonth = useMemo(() => {
    if (!report) return 0;
    return report.months.reduce(
      (peak, month) => Math.max(peak, month.incomeMinor, month.expenseMinor),
      0,
    );
  }, [report]);

  const slices = breakdown === "expense" ? report?.expenseCategories ?? [] : report?.incomeCategories ?? [];
  const sliceTotal = slices.reduce((total, slice) => total + slice.amountMinor, 0);

  const applyRange = (option: RangeOption) => {
    const next = option.build();
    setRangeId(option.id);
    setFrom(next.from);
    setTo(next.to);
  };

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Patterns, from recorded rows
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Analytics</h2>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
            Every figure is aggregated in Rust from effective transactions only. Transfers, investment moves,
            opening balances, adjustments, pending rows and reversed rows are excluded — they do not describe
            income or spending.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          {RANGES.map((option) => (
            <Button
              key={option.id}
              size="sm"
              variant={rangeId === option.id ? "primary" : "ghost"}
              onClick={() => applyRange(option)}
            >
              {option.label}
            </Button>
          ))}
        </div>
      </div>

      <Panel className="flex flex-wrap items-end gap-3 p-4">
        <label className="block">
          <span className="rawjan-label">From</span>
          <input
            type="date"
            className="rawjan-input font-mono"
            value={from}
            max={to}
            onChange={(event) => {
              setRangeId("custom");
              setFrom(event.target.value);
            }}
          />
        </label>
        <label className="block">
          <span className="rawjan-label">To</span>
          <input
            type="date"
            className="rawjan-input font-mono"
            value={to}
            min={from}
            onChange={(event) => {
              setRangeId("custom");
              setTo(event.target.value);
            }}
          />
        </label>
        <div className="ml-auto flex items-center gap-2 text-[0.62rem] text-zinc-600">
          <Badge tone="champagne">{report?.currency ?? app.primaryCurrency}</Badge>
          {report ? (
            <span>
              {report.summary.transactionCount} effective transaction
              {report.summary.transactionCount === 1 ? "" : "s"} · {report.summary.monthCount} month
              {report.summary.monthCount === 1 ? "" : "s"}
            </span>
          ) : null}
        </div>
      </Panel>

      {reportQuery.isError ? (
        <Alert tone="error" title="Could not load analytics">
          {errorMessage(reportQuery.error)}
        </Alert>
      ) : null}

      {reportQuery.isLoading || !report ? (
        <div className="flex items-center justify-center py-16 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : (
        <>
          <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <SummaryCard
              label="Income"
              value={money(report.summary.income, report.currency)}
              tone="text-sage-400"
              icon={<ArrowUpRight className="h-3.5 w-3.5" />}
            />
            <SummaryCard
              label="Expense"
              value={money(report.summary.expense, report.currency)}
              tone="text-zinc-100"
              icon={<ArrowDownRight className="h-3.5 w-3.5" />}
            />
            <SummaryCard
              label="Net"
              value={money(report.summary.net, report.currency)}
              tone={report.summary.netMinor >= 0 ? "text-sage-400" : "text-lily-300"}
              icon={<ChartColumn className="h-3.5 w-3.5" />}
            />
            <SummaryCard
              label="Savings rate"
              value={
                report.summary.savingsRate === null
                  ? "Not measurable"
                  : `${report.summary.savingsRate}%`
              }
              tone="text-champagne-300"
              hint={`Avg. monthly expense ${money(report.summary.averageMonthlyExpense, report.currency)}`}
            />
          </div>

          <Panel className="p-5">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <SectionLabel>Income vs expense by month</SectionLabel>
              <div className="flex items-center gap-3 text-[0.6rem] text-zinc-500">
                <span className="inline-flex items-center gap-1.5">
                  <span className="h-2 w-2 rounded-sm bg-sage-500/80" /> Income
                </span>
                <span className="inline-flex items-center gap-1.5">
                  <span className="h-2 w-2 rounded-sm bg-lily-500/80" /> Expense
                </span>
              </div>
            </div>

            {report.months.length === 0 || maxMonth <= 0 ? (
              <EmptyState
                className="mt-3"
                icon={<ChartColumn className="h-5 w-5" />}
                title="No effective activity in this range"
                description="Record or post transactions, or widen the date range. Nothing is estimated or projected here."
              />
            ) : (
              <div className="mt-5 overflow-x-auto pb-1">
                <div className="flex min-w-full items-end gap-3" style={{ height: 180 }}>
                  {report.months.map((month) => {
                    const incomeHeight = maxMonth > 0 ? (month.incomeMinor / maxMonth) * 100 : 0;
                    const expenseHeight = maxMonth > 0 ? (month.expenseMinor / maxMonth) * 100 : 0;
                    return (
                      <div key={month.month} className="flex min-w-[52px] flex-1 flex-col items-center gap-1">
                        <div className="flex h-[130px] w-full items-end justify-center gap-1">
                          <div
                            className="w-[42%] rounded-t-sm bg-sage-500/70 transition-all"
                            style={{ height: `${Math.max(incomeHeight, month.incomeMinor > 0 ? 2 : 0)}%` }}
                            title={`Income ${money(month.income, report.currency)}`}
                          />
                          <div
                            className="w-[42%] rounded-t-sm bg-lily-500/70 transition-all"
                            style={{ height: `${Math.max(expenseHeight, month.expenseMinor > 0 ? 2 : 0)}%` }}
                            title={`Expense ${money(month.expense, report.currency)}`}
                          />
                        </div>
                        <div className="text-[0.58rem] uppercase tracking-wider text-zinc-600">
                          {month.label}
                        </div>
                        <div
                          className={clsx(
                            "font-mono text-[0.58rem]",
                            month.netMinor >= 0 ? "text-zinc-500" : "text-lily-300/80",
                          )}
                        >
                          {month.net}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
            <div className="mt-3 text-[0.63rem] leading-relaxed text-zinc-600">
              Bars show posted and reconciled amounts converted to {report.currency} using stored exchange-rate
              snapshots. Months with no activity are still shown so gaps are visible.
            </div>
          </Panel>

          <div className="grid gap-3 lg:grid-cols-[1.2fr_1fr]">
            <Panel className="p-5">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <SectionLabel>Category breakdown</SectionLabel>
                <div className="flex rounded-lg border border-white/[0.07] p-0.5">
                  {(["expense", "income"] as const).map((option) => (
                    <button
                      key={option}
                      type="button"
                      onClick={() => setBreakdown(option)}
                      className={clsx(
                        "rounded-md px-2.5 py-1 text-[0.62rem] capitalize transition",
                        breakdown === option
                          ? "bg-white/[0.07] text-zinc-100"
                          : "text-zinc-500 hover:text-zinc-300",
                      )}
                    >
                      {option}
                    </button>
                  ))}
                </div>
              </div>

              {slices.length === 0 ? (
                <EmptyState
                  className="mt-3"
                  icon={<ChartColumn className="h-5 w-5" />}
                  title={`No ${breakdown} categories in range`}
                  description="Categorised ledger entries will appear here. Uncategorised amounts are not invented into a bucket."
                />
              ) : (
                <div className="mt-4 space-y-3">
                  {slices.map((slice) => {
                    const share = percentOf(slice.amountMinor, sliceTotal);
                    return (
                      <div key={slice.categoryId}>
                        <div className="flex items-center justify-between gap-3 text-[0.68rem]">
                          <span className="inline-flex min-w-0 items-center gap-2 text-zinc-300">
                            <span
                              className="h-2 w-2 shrink-0 rounded-full"
                              style={{ backgroundColor: slice.color ?? "#7A6A8E" }}
                            />
                            <span className="truncate">{slice.name}</span>
                            <span className="shrink-0 text-[0.58rem] text-zinc-600">
                              {slice.transactionCount} txn{slice.transactionCount === 1 ? "" : "s"}
                            </span>
                          </span>
                          <span className="shrink-0 font-mono text-[0.64rem] text-zinc-500">
                            {slice.amount}
                          </span>
                        </div>
                        <div className="mt-1.5 flex items-center gap-2">
                          <div className="h-1 flex-1 overflow-hidden rounded-full bg-white/[0.05]">
                            <div
                              className="h-full rounded-full bg-champagne-500/70"
                              style={{ width: `${Math.max(share, 1)}%` }}
                            />
                          </div>
                          <span className="w-8 text-right text-[0.58rem] text-zinc-600">{slice.percent}%</span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
              <div className="mt-4 border-t border-white/[0.05] pt-3 text-[0.62rem] text-zinc-600">
                Top 12 categories. Percentages are computed in Rust against the range total.
              </div>
            </Panel>

            <Panel className="p-5">
              <div className="flex items-center justify-between">
                <SectionLabel>Top payees</SectionLabel>
                <Badge tone="muted">expense only</Badge>
              </div>
              {report.payees.length === 0 ? (
                <EmptyState
                  className="mt-3"
                  icon={<Users className="h-5 w-5" />}
                  title="No payees recorded"
                  description="Add a payee when entering an expense. The description is used only when no payee exists."
                />
              ) : (
                <div className="mt-3 space-y-2.5">
                  {report.payees.map((payee) => (
                    <div
                      key={payee.label}
                      className="flex items-center justify-between gap-3 border-b border-white/[0.04] pb-2 last:border-0 last:pb-0"
                    >
                      <div className="min-w-0">
                        <div className="truncate text-[0.72rem] text-zinc-300">{payee.label}</div>
                        <div className="text-[0.58rem] text-zinc-600">
                          {payee.count} transaction{payee.count === 1 ? "" : "s"}
                        </div>
                      </div>
                      <div className="shrink-0 font-mono text-[0.68rem] text-zinc-400">
                        {payee.amount}
                      </div>
                    </div>
                  ))}
                </div>
              )}
              <div className="mt-4 border-t border-white/[0.05] pt-3 text-[0.62rem] leading-relaxed text-zinc-600">
                Range: {formatDate(report.from)} – {formatDate(report.to)}. Amounts are in{" "}
                {report.currency}; no post-hoc conversion is applied to the display.
              </div>
            </Panel>
          </div>
        </>
      )}
    </div>
  );
}

function SummaryCard({
  label,
  value,
  tone,
  icon,
  hint,
}: {
  label: string;
  value: string;
  tone: string;
  icon?: ReactNode;
  hint?: string;
}) {
  return (
    <Panel className="p-4">
      <div className="flex items-center justify-between text-[0.58rem] uppercase tracking-[0.16em] text-zinc-600">
        <span>{label}</span>
        {icon ? <span className="text-zinc-600">{icon}</span> : null}
      </div>
      <div className={clsx("mt-2 font-display text-xl leading-none", tone)}>{value}</div>
      {hint ? <div className="mt-1.5 text-[0.58rem] text-zinc-600">{hint}</div> : null}
    </Panel>
  );
}
