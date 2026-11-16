import { useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import {
  ArrowDownRight,
  ArrowUpRight,
  ChartColumn,
  Coins,
  Pencil,
  Plus,
  Wallet,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { IncomeSourceReport } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { formatDate, money } from "@/lib/display";
import { useToast } from "@/components/Toast";
import {
  Alert,
  Badge,
  Button,
  EmptyState,
  Field,
  Modal,
  Panel,
  SectionLabel,
  Spinner,
} from "@/components/ui";

const SOURCE_TYPES = [
  { value: "salary", label: "Salary" },
  { value: "bonus", label: "Bonus" },
  { value: "business", label: "Business" },
  { value: "freelance", label: "Freelance" },
  { value: "dividend", label: "Dividend" },
  { value: "rental", label: "Rental" },
  { value: "interest", label: "Interest" },
  { value: "gift", label: "Gift" },
  { value: "other", label: "Other" },
];

const SCHEDULES = [
  { value: "monthly", label: "Monthly" },
  { value: "semimonthly", label: "Twice a month" },
  { value: "biweekly", label: "Every two weeks" },
  { value: "weekly", label: "Weekly" },
  { value: "irregular", label: "Irregular" },
  { value: "one_time", label: "One time" },
];

function labelFor(options: { value: string; label: string }[], value: string): string {
  return options.find((option) => option.value === value)?.label ?? value;
}

function firstDayOfMonth(date: Date, offset: number): string {
  const shifted = new Date(date.getFullYear(), date.getMonth() + offset, 1);
  const month = String(shifted.getMonth() + 1).padStart(2, "0");
  return `${shifted.getFullYear()}-${month}-01`;
}

function lastDayOfMonth(date: Date, offset: number): string {
  const shifted = new Date(date.getFullYear(), date.getMonth() + offset + 1, 0);
  const month = String(shifted.getMonth() + 1).padStart(2, "0");
  const day = String(shifted.getDate()).padStart(2, "0");
  return `${shifted.getFullYear()}-${month}-${day}`;
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
    id: "this_month",
    label: "This month",
    build: () => ({ from: firstDayOfMonth(new Date(), 0), to: todayInput() }),
  },
  {
    id: "last_month",
    label: "Last month",
    build: () => ({ from: firstDayOfMonth(new Date(), -1), to: lastDayOfMonth(new Date(), -1) }),
  },
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
    build: () => ({ from: `${new Date().getFullYear()}-01-01`, to: todayInput() }),
  },
];

interface SourceFormState {
  name: string;
  sourceType: string;
  currencyCode: string;
  expectedAmount: string;
  schedule: string;
  payDay: string;
  accountId: string;
  categoryId: string;
  isActive: boolean;
  notes: string;
}

function emptyForm(currency: string): SourceFormState {
  return {
    name: "",
    sourceType: "salary",
    currencyCode: currency,
    expectedAmount: "",
    schedule: "monthly",
    payDay: "",
    accountId: "",
    categoryId: "",
    isActive: true,
    notes: "",
  };
}

export function IncomePage() {
  const { app, accounts, categories, currencies } = useBootstrap();
  const toast = useToast();
  const queryClient = useQueryClient();

  const [rangeId, setRangeId] = useState("6m");
  const [from, setFrom] = useState(() => RANGES[3].build().from);
  const [to, setTo] = useState(() => RANGES[3].build().to);

  const [sourceOpen, setSourceOpen] = useState(false);
  const [editing, setEditing] = useState<IncomeSourceReport | null>(null);
  const [form, setForm] = useState<SourceFormState>(() => emptyForm(app.primaryCurrency));
  const [formError, setFormError] = useState<string | null>(null);

  const reportQuery = useQuery({
    queryKey: ["income", from, to],
    queryFn: () => api.incomeReport(from, to),
  });
  const report = reportQuery.data;

  const activeAccounts = useMemo(() => accounts.filter((account) => account.isActive), [accounts]);
  const incomeCategories = useMemo(
    () =>
      categories
        .filter((category) => category.kind === "income" && !category.isArchived)
        .sort((a, b) => {
          if (a.parentId && !b.parentId) return 1;
          if (!a.parentId && b.parentId) return -1;
          return a.name.localeCompare(b.name);
        }),
    [categories],
  );

  const maxMonth = useMemo(() => {
    if (!report) return 0;
    return report.months.reduce(
      (peak, month) => Math.max(peak, month.incomeMinor, month.expenseMinor),
      0,
    );
  }, [report]);

  const hasActivity = useMemo(
    () => (report ? report.months.some((month) => month.incomeMinor > 0 || month.expenseMinor > 0) : false),
    [report],
  );

  const applyRange = (option: RangeOption) => {
    const next = option.build();
    setRangeId(option.id);
    setFrom(next.from);
    setTo(next.to);
  };

  const openAddSource = () => {
    setEditing(null);
    setForm(emptyForm(app.primaryCurrency));
    setFormError(null);
    setSourceOpen(true);
  };

  const openEditSource = (source: IncomeSourceReport) => {
    setEditing(source);
    setForm({
      name: source.name,
      sourceType: source.sourceType,
      currencyCode: source.currency,
      expectedAmount: source.expectedAmount,
      schedule: source.schedule,
      payDay: source.payDay === null ? "" : String(source.payDay),
      accountId: source.accountId ?? "",
      categoryId: source.categoryId ?? "",
      isActive: source.isActive,
      notes: source.notes ?? "",
    });
    setFormError(null);
    setSourceOpen(true);
  };

  const saveSource = useMutation({
    mutationFn: () =>
      api.saveIncomeSource(editing?.id ?? null, {
        name: form.name.trim(),
        sourceType: form.sourceType,
        currencyCode: form.currencyCode,
        expectedAmount: form.expectedAmount.trim() || null,
        schedule: form.schedule,
        payDay: form.payDay.trim() ? Number(form.payDay) : null,
        accountId: form.accountId || null,
        categoryId: form.categoryId || null,
        isActive: form.isActive,
        notes: form.notes.trim() || null,
      }),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["income"] }),
        queryClient.invalidateQueries({ queryKey: ["income-sources"] }),
      ]);
      toast.success(editing ? "Income source updated" : "Income source saved");
      setSourceOpen(false);
    },
    onError: (error) => setFormError(errorMessage(error)),
  });

  const submitSource = () => {
    setFormError(null);
    if (!form.name.trim()) {
      setFormError("Give the income source a name.");
      return;
    }
    if (form.payDay.trim()) {
      const day = Number(form.payDay);
      if (!Number.isInteger(day) || day < 1 || day > 31) {
        setFormError("Pay day must be a whole number between 1 and 31.");
        return;
      }
    }
    saveSource.mutate();
  };

  const needsPayDay = form.schedule === "monthly" || form.schedule === "semimonthly";
  const change = report?.summary.changePercent ?? null;

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            What the ledger earned
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Income</h2>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
            Income received, income sources and the gap between what you expected and what arrived. Every figure
            is read from posted or reconciled entries; pending, draft and reversed rows never count.
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
              {report.summary.transactionCount} income entr
              {report.summary.transactionCount === 1 ? "y" : "ies"} · {report.summary.monthCount} month
              {report.summary.monthCount === 1 ? "" : "s"}
            </span>
          ) : null}
        </div>
      </Panel>

      {reportQuery.isError ? (
        <Alert tone="error" title="Could not load income">
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
              label="Income received"
              value={money(report.summary.income, report.currency)}
              tone="text-sage-400"
              icon={<ArrowUpRight className="h-3.5 w-3.5" />}
              hint={
                report.summary.previousIncomeMinor > 0
                  ? `${change !== null && change > 0 ? "+" : ""}${change ?? 0}% vs ${money(
                      report.summary.previousIncome,
                      report.currency,
                    )} in the previous period`
                  : "No income in the previous period"
              }
            />
            <SummaryCard
              label="Average per month"
              value={money(report.summary.averageMonthly, report.currency)}
              tone="text-zinc-100"
              icon={<ChartColumn className="h-3.5 w-3.5" />}
              hint={`Across ${report.summary.monthCount} month${report.summary.monthCount === 1 ? "" : "s"}`}
            />
            <SummaryCard
              label="Net cash flow"
              value={money(report.summary.net, report.currency)}
              tone={report.summary.netMinor >= 0 ? "text-sage-400" : "text-lily-300"}
              icon={<ArrowDownRight className="h-3.5 w-3.5" />}
              hint={`Expenses ${money(report.summary.expense, report.currency)}`}
            />
            <SummaryCard
              label="Savings rate"
              value={report.summary.savingsRate === null ? "Not measurable" : `${report.summary.savingsRate}%`}
              tone="text-champagne-300"
              icon={<Coins className="h-3.5 w-3.5" />}
              hint={
                report.summary.unassignedCount > 0
                  ? `${money(report.summary.unassigned, report.currency)} not linked to a source`
                  : "Every income entry is linked to a source"
              }
            />
          </div>

          {report.summary.unassignedCount > 0 ? (
            <Alert tone="warning" title="Income without a source">
              {report.summary.unassignedCount} income entr
              {report.summary.unassignedCount === 1 ? "y is" : "ies are"} worth{" "}
              {money(report.summary.unassigned, report.currency)} in this range but{" "}
              {report.summary.unassignedCount === 1 ? "is" : "are"} not linked to an income source. Link the source
              when recording income to compare expected and received amounts.
            </Alert>
          ) : null}

          <Panel className="p-5">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <SectionLabel>Income and spending by month</SectionLabel>
              <div className="flex items-center gap-3 text-[0.6rem] text-zinc-500">
                <span className="inline-flex items-center gap-1.5">
                  <span className="h-2 w-2 rounded-sm bg-sage-500/80" /> Income
                </span>
                <span className="inline-flex items-center gap-1.5">
                  <span className="h-2 w-2 rounded-sm bg-lily-500/80" /> Expense
                </span>
              </div>
            </div>

            {!hasActivity || maxMonth <= 0 ? (
              <EmptyState
                className="mt-3"
                icon={<ChartColumn className="h-5 w-5" />}
                title="No effective activity in this range"
                description="Record or post income and expenses, or widen the date range. Nothing is estimated or projected here."
              />
            ) : (
              <div className="mt-5 overflow-x-auto pb-1">
                <div className="flex min-w-full items-end gap-3" style={{ height: 180 }}>
                  {report.months.map((month) => {
                    const incomeHeight = maxMonth > 0 ? (month.incomeMinor / maxMonth) * 100 : 0;
                    const expenseHeight = maxMonth > 0 ? (month.expenseMinor / maxMonth) * 100 : 0;
                    return (
                      <div key={month.month} className="flex min-w-[52px] flex-1 flex-col items-center gap-1">
                        <div className="flex h-[140px] w-full items-end justify-center gap-1">
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
                        <div className="text-[0.56rem] uppercase tracking-[0.08em] text-zinc-600">
                          {month.label}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </Panel>

          <div className="grid gap-4 xl:grid-cols-[1.65fr_1fr]">
            <Panel className="p-5">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <SectionLabel>Income by source</SectionLabel>
                <Button size="sm" variant="secondary" onClick={openAddSource}>
                  <Plus className="h-3.5 w-3.5" /> Add source
                </Button>
              </div>

              {report.sources.length === 0 ? (
                <EmptyState
                  className="mt-3"
                  icon={<Coins className="h-5 w-5" />}
                  title="No income sources yet"
                  description="A source records what you expect, how often it arrives and which account receives it. It is optional, but it is what makes expected-versus-received reporting possible."
                  action={
                    <Button variant="primary" size="sm" onClick={openAddSource}>
                      <Plus className="h-3.5 w-3.5" /> Add the first source
                    </Button>
                  }
                />
              ) : (
                <div className="mt-3 overflow-x-auto">
                  <table className="w-full min-w-[720px] border-collapse text-left">
                    <thead>
                      <tr className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">
                        <th scope="col" className="pb-2 pr-3 font-medium">
                          Source
                        </th>
                        <th scope="col" className="pb-2 pr-3 font-medium">
                          Expected
                        </th>
                        <th scope="col" className="pb-2 pr-3 text-right font-medium">
                          Received
                        </th>
                        <th scope="col" className="pb-2 pr-3 text-right font-medium">
                          Variance
                        </th>
                        <th scope="col" className="pb-2 pr-3 font-medium">
                          Last received
                        </th>
                        <th scope="col" className="pb-2 text-right font-medium">
                          <span className="sr-only">Actions</span>
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {report.sources.map((source) => (
                        <tr key={source.id} className="border-t border-white/[0.05] align-top">
                          <td className="py-2.5 pr-3">
                            <div className="flex items-center gap-1.5">
                              <span className="text-[0.74rem] text-zinc-200">{source.name}</span>
                              {source.isActive ? null : <Badge tone="muted">Paused</Badge>}
                            </div>
                            <div className="text-[0.58rem] text-zinc-600">
                              {labelFor(SOURCE_TYPES, source.sourceType)} ·{" "}
                              {labelFor(SCHEDULES, source.schedule)}
                              {source.payDay ? ` · day ${source.payDay}` : ""}
                            </div>
                            <div className="text-[0.58rem] text-zinc-600">
                              {source.accountName ?? "No account linked"}
                              {source.categoryName ? ` · ${source.categoryName}` : ""}
                            </div>
                          </td>
                          <td className="py-2.5 pr-3">
                            <div className="font-mono text-[0.68rem] text-zinc-300">
                              {source.expected === null
                                ? "—"
                                : money(source.expected, report.currency)}
                            </div>
                            {source.expected !== null && source.occurrences !== null ? (
                              <div className="text-[0.56rem] text-zinc-600">
                                {source.occurrences} × {source.expectedAmount} {source.currency}
                              </div>
                            ) : null}
                            {source.estimateNote ? (
                              <div className="mt-1 max-w-[220px] text-[0.56rem] leading-snug text-zinc-600">
                                {source.estimateNote}
                              </div>
                            ) : null}
                          </td>
                          <td className="py-2.5 pr-3 text-right font-mono text-[0.68rem] text-zinc-300">
                            {money(source.received, report.currency)}
                          </td>
                          <td
                            className={clsx(
                              "py-2.5 pr-3 text-right font-mono text-[0.68rem]",
                              source.varianceMinor === null
                                ? "text-zinc-600"
                                : source.varianceMinor < 0
                                  ? "text-lily-300"
                                  : "text-sage-400",
                            )}
                          >
                            {source.variance === null
                              ? "—"
                              : `${source.varianceMinor !== null && source.varianceMinor > 0 ? "+" : ""}${money(
                                  source.variance,
                                  report.currency,
                                )}`}
                          </td>
                          <td className="py-2.5 pr-3 text-[0.64rem] text-zinc-500">
                            {source.lastReceivedDate ? formatDate(source.lastReceivedDate) : "—"}
                          </td>
                          <td className="py-2.5 text-right">
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() => openEditSource(source)}
                              aria-label={`Edit ${source.name}`}
                            >
                              <Pencil className="h-3 w-3" /> Edit
                            </Button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}

              <div className="mt-4 border-t border-white/[0.05] pt-3 text-[0.62rem] leading-relaxed text-zinc-600">
                Expected totals are estimates from the saved amount, schedule and pay day. They are never converted
                between currencies, and weekly, biweekly and irregular sources are not estimated at all. Nothing
                here is a forecast of future income.
              </div>
            </Panel>

            <div className="space-y-4">
              <Panel className="p-5">
                <div className="flex items-center justify-between">
                  <SectionLabel>Income by account</SectionLabel>
                  <Badge tone="muted">{report.currency} base</Badge>
                </div>
                {report.accounts.length === 0 ? (
                  <EmptyState
                    className="mt-3"
                    icon={<Wallet className="h-5 w-5" />}
                    title="No income recorded"
                    description="Income appears here once an effective income entry is posted to an account."
                  />
                ) : (
                  <div className="mt-3 space-y-2.5">
                    {report.accounts.map((account) => (
                      <div key={account.accountId} className="border-b border-white/[0.04] pb-2 last:border-0 last:pb-0">
                        <div className="flex items-center justify-between gap-3">
                          <div className="min-w-0">
                            <div className="truncate text-[0.72rem] text-zinc-300">{account.name}</div>
                            <div className="text-[0.58rem] text-zinc-600">
                              {account.transactionCount} entr{account.transactionCount === 1 ? "y" : "ies"} ·{" "}
                              {account.percent}% of income
                            </div>
                          </div>
                          <div className="shrink-0 font-mono text-[0.68rem] text-zinc-400">{account.income}</div>
                        </div>
                        <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-white/[0.04]">
                          <div
                            className="h-full rounded-full bg-sage-500/70"
                            style={{ width: `${Math.min(account.percent, 100)}%` }}
                          />
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </Panel>

              <Panel className="p-5">
                <div className="flex items-center justify-between">
                  <SectionLabel>Income by category</SectionLabel>
                  <Badge tone="muted">{report.incomeCategories.length} shown</Badge>
                </div>
                {report.incomeCategories.length === 0 ? (
                  <EmptyState
                    className="mt-3"
                    icon={<Coins className="h-5 w-5" />}
                    title="No income categories"
                    description="Categorise income entries to see which kinds of income carry the range."
                  />
                ) : (
                  <div className="mt-3 space-y-2.5">
                    {report.incomeCategories.map((slice) => (
                      <div key={slice.categoryId} className="border-b border-white/[0.04] pb-2 last:border-0 last:pb-0">
                        <div className="flex items-center justify-between gap-3">
                          <div className="flex min-w-0 items-center gap-2">
                            <span
                              className="h-2 w-2 shrink-0 rounded-sm"
                              style={{ backgroundColor: slice.color ?? "#6A6A6E" }}
                            />
                            <div className="min-w-0">
                              <div className="truncate text-[0.72rem] text-zinc-300">{slice.name}</div>
                              <div className="text-[0.58rem] text-zinc-600">
                                {slice.transactionCount} entr{slice.transactionCount === 1 ? "y" : "ies"} ·{" "}
                                {slice.percent}%
                              </div>
                            </div>
                          </div>
                          <div className="shrink-0 font-mono text-[0.68rem] text-zinc-400">{slice.amount}</div>
                        </div>
                        <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-white/[0.04]">
                          <div
                            className="h-full rounded-full bg-champagne-500/60"
                            style={{ width: `${Math.min(slice.percent, 100)}%` }}
                          />
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </Panel>
            </div>
          </div>

          <div className="text-[0.62rem] leading-relaxed text-zinc-600">
            Range: {formatDate(report.from)} – {formatDate(report.to)}. Amounts are in {report.currency}. The
            previous-period comparison uses the immediately preceding range of the same length, or the previous
            calendar month(s) when the range is month-aligned.
          </div>
        </>
      )}

      <Modal
        open={sourceOpen}
        onClose={() => setSourceOpen(false)}
        title={editing ? "Edit income source" : "New income source"}
        description="A source is your own record of what you expect. It never posts anything to the ledger on its own."
        width="lg"
        footer={
          <>
            <Button variant="ghost" onClick={() => setSourceOpen(false)} disabled={saveSource.isPending}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submitSource} loading={saveSource.isPending}>
              {editing ? "Save changes" : "Save source"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          {formError ? (
            <Alert tone="error" title="Could not save the source">
              {formError}
            </Alert>
          ) : null}

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Name">
              <input
                className="rawjan-input"
                placeholder="Main salary"
                value={form.name}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
                autoFocus
              />
            </Field>
            <Field label="Type">
              <select
                className="rawjan-input"
                value={form.sourceType}
                onChange={(event) => setForm({ ...form, sourceType: event.target.value })}
              >
                {SOURCE_TYPES.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Currency">
              <select
                className="rawjan-input"
                value={form.currencyCode}
                onChange={(event) => setForm({ ...form, currencyCode: event.target.value })}
              >
                {currencies.map((currency) => (
                  <option key={currency.code} value={currency.code}>
                    {currency.code} · {currency.name}
                  </option>
                ))}
              </select>
            </Field>
            <Field
              label="Expected amount"
              hint="Per payout, in the source currency. Optional."
            >
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={form.expectedAmount}
                onChange={(event) => setForm({ ...form, expectedAmount: event.target.value })}
              />
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Schedule">
              <select
                className="rawjan-input"
                value={form.schedule}
                onChange={(event) => setForm({ ...form, schedule: event.target.value })}
              >
                {SCHEDULES.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </Field>
            {needsPayDay ? (
              <Field label="Pay day" hint="Optional. The day of the month you expect it.">
                <input
                  className="rawjan-input font-mono"
                  inputMode="numeric"
                  placeholder="e.g. 28"
                  value={form.payDay}
                  onChange={(event) => setForm({ ...form, payDay: event.target.value })}
                />
              </Field>
            ) : (
              <div className="hidden sm:block" />
            )}
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Account" hint="Optional. Where the income normally lands.">
              <select
                className="rawjan-input"
                value={form.accountId}
                onChange={(event) => setForm({ ...form, accountId: event.target.value })}
              >
                <option value="">No account linked</option>
                {activeAccounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name} · {account.currency}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Category" hint="Optional. Used as the default when recording income.">
              <select
                className="rawjan-input"
                value={form.categoryId}
                onChange={(event) => setForm({ ...form, categoryId: event.target.value })}
              >
                <option value="">No category linked</option>
                {incomeCategories.map((category) => (
                  <option key={category.id} value={category.id}>
                    {category.parentId ? `— ${category.name}` : category.name}
                  </option>
                ))}
              </select>
            </Field>
          </div>

          <Field label="Notes" hint="Optional.">
            <textarea
              className="rawjan-input min-h-[72px] resize-y"
              placeholder="Contract terms, review date, anything worth remembering"
              value={form.notes}
              onChange={(event) => setForm({ ...form, notes: event.target.value })}
            />
          </Field>

          <label className="flex items-center gap-2 text-[0.7rem] text-zinc-400">
            <input
              type="checkbox"
              className="h-4 w-4 rounded border-white/20 bg-black/40 accent-lily-500"
              checked={form.isActive}
              onChange={(event) => setForm({ ...form, isActive: event.target.checked })}
            />
            Active source
          </label>
        </div>
      </Modal>
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
