import { useMemo, useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import clsx from "clsx";
import {
  ArrowDownLeft,
  ArrowLeftRight,
  ArrowUpRight,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Filter,
  Plus,
  Receipt,
  RefreshCw,
  Search,
  X,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { TransactionFilters } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { amountTone, formatDate, signedAmount, STATUS_LABELS } from "@/lib/display";
import { useComposer } from "@/components/ComposerContext";
import { TransactionDetailModal, statusTone } from "@/components/TransactionDetail";
import { Alert, Badge, Button, EmptyState, Panel, Spinner } from "@/components/ui";

const PAGE_SIZE = 50;

const STATUS_FILTERS = [
  { value: "effective", label: "Posted & reconciled" },
  { value: "needs_review", label: "Needs review" },
  { value: "", label: "All statuses" },
  { value: "draft", label: "Drafts" },
  { value: "pending", label: "Pending" },
  { value: "posted", label: "Posted" },
  { value: "reconciled", label: "Reconciled" },
  { value: "reversed", label: "Reversed" },
  { value: "cancelled", label: "Cancelled" },
];

const KINDS = [
  { value: "", label: "All kinds" },
  { value: "expense", label: "Expense" },
  { value: "income", label: "Income" },
  { value: "transfer", label: "Transfer" },
  { value: "refund", label: "Refund" },
  { value: "fee", label: "Fee" },
  { value: "investment_buy", label: "Investment buy" },
  { value: "investment_sell", label: "Investment sell" },
  { value: "dividend", label: "Dividend" },
  { value: "adjustment", label: "Adjustment" },
  { value: "opening", label: "Opening balance" },
];

export function TransactionsPage() {
  const { accounts, categories, app } = useBootstrap();
  const { openComposer } = useComposer();
  const [detailId, setDetailId] = useState<string | null>(null);
  const [showFilters, setShowFilters] = useState(false);
  const [search, setSearch] = useState("");
  const [appliedSearch, setAppliedSearch] = useState("");
  const [accountId, setAccountId] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const [kind, setKind] = useState("");
  const [status, setStatus] = useState("effective");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [page, setPage] = useState(0);

  const filters: TransactionFilters = useMemo(
    () => ({
      search: appliedSearch || undefined,
      accountId: accountId || undefined,
      categoryId: categoryId || undefined,
      kind: kind || undefined,
      status: status || undefined,
      from: from || undefined,
      to: to || undefined,
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
    }),
    [appliedSearch, accountId, categoryId, kind, status, from, to, page],
  );

  const list = useQuery({
    queryKey: ["transactions", filters],
    queryFn: () => api.listTransactions(filters),
    placeholderData: keepPreviousData,
  });

  const expenseCategories = categories.filter((category) => category.kind === "expense" && !category.isArchived);
  const incomeCategories = categories.filter((category) => category.kind === "income" && !category.isArchived);

  const rows = list.data?.rows ?? [];
  const total = list.data?.total ?? 0;
  const pageCount = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const hasFilters = Boolean(
    appliedSearch || accountId || categoryId || kind || from || to || status !== "effective",
  );

  const submitSearch = () => {
    setPage(0);
    setAppliedSearch(search.trim());
  };

  const resetFilters = () => {
    setSearch("");
    setAppliedSearch("");
    setAccountId("");
    setCategoryId("");
    setKind("");
    setStatus("effective");
    setFrom("");
    setTo("");
    setPage(0);
  };

  const applyFilter = <T,>(setter: (value: T) => void, value: T) => {
    setter(value);
    setPage(0);
  };

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            General ledger
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">
            Transactions
          </h2>
          <p className="mt-1 max-w-2xl text-xs leading-relaxed text-zinc-500">
            Every record is a balanced double-entry transaction. Posted rows are immutable; corrections use
            reversals so the audit trail stays intact.
          </p>
        </div>
        <Button size="sm" variant="primary" onClick={() => openComposer("expense")}>
          <Plus className="h-3.5 w-3.5" /> New entry
        </Button>
      </div>

      <Panel className="overflow-hidden">
        <div className="flex flex-wrap items-center gap-2 border-b border-white/[0.055] px-4 py-3">
          <div className="relative min-w-[200px] flex-1">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-zinc-600" />
            <input
              className="rawjan-input h-9 pl-8 text-xs"
              placeholder="Search description, payee or notes…"
              aria-label="Search transactions"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") submitSearch();
              }}
            />
            {search ? (
              <button
                type="button"
                className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-0.5 text-zinc-600 hover:text-zinc-300"
                onClick={() => {
                  setSearch("");
                  setAppliedSearch("");
                  setPage(0);
                }}
                aria-label="Clear search"
              >
                <X className="h-3 w-3" />
              </button>
            ) : null}
          </div>
          <Button size="sm" variant="secondary" onClick={submitSearch}>
            Search
          </Button>
          <Button
            size="sm"
            variant={showFilters ? "primary" : "secondary"}
            onClick={() => setShowFilters((value) => !value)}
          >
            <Filter className="h-3.5 w-3.5" /> Filters
            {hasFilters ? <span className="h-1.5 w-1.5 rounded-full bg-lily-300" /> : null}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void list.refetch()}
            disabled={list.isFetching}
            aria-label="Refresh"
          >
            <RefreshCw className={clsx("h-3.5 w-3.5", list.isFetching && "animate-spin")} />
          </Button>
        </div>

        {showFilters ? (
          <div className="grid gap-3 border-b border-white/[0.055] bg-black/20 px-4 py-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
            <label className="block">
              <span className="rawjan-label">Status</span>
              <select
                className="rawjan-input h-9 text-xs"
                value={status}
                onChange={(event) => applyFilter(setStatus, event.target.value)}
              >
                {STATUS_FILTERS.map((option) => (
                  <option key={option.value || "all"} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="block">
              <span className="rawjan-label">Account</span>
              <select
                className="rawjan-input h-9 text-xs"
                value={accountId}
                onChange={(event) => applyFilter(setAccountId, event.target.value)}
              >
                <option value="">All accounts</option>
                {accounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name} · {account.currency}
                  </option>
                ))}
              </select>
            </label>
            <label className="block">
              <span className="rawjan-label">Category</span>
              <select
                className="rawjan-input h-9 text-xs"
                value={categoryId}
                onChange={(event) => applyFilter(setCategoryId, event.target.value)}
              >
                <option value="">All categories</option>
                <optgroup label="Expenses">
                  {expenseCategories.map((category) => (
                    <option key={category.id} value={category.id}>
                      {category.name}
                    </option>
                  ))}
                </optgroup>
                <optgroup label="Income">
                  {incomeCategories.map((category) => (
                    <option key={category.id} value={category.id}>
                      {category.name}
                    </option>
                  ))}
                </optgroup>
              </select>
            </label>
            <label className="block">
              <span className="rawjan-label">Kind</span>
              <select
                className="rawjan-input h-9 text-xs"
                value={kind}
                onChange={(event) => applyFilter(setKind, event.target.value)}
              >
                {KINDS.map((option) => (
                  <option key={option.value || "all"} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="block">
              <span className="rawjan-label">From</span>
              <input
                type="date"
                className="rawjan-input h-9 text-xs"
                value={from}
                onChange={(event) => applyFilter(setFrom, event.target.value)}
              />
            </label>
            <label className="block">
              <span className="rawjan-label">To</span>
              <input
                type="date"
                className="rawjan-input h-9 text-xs"
                value={to}
                onChange={(event) => applyFilter(setTo, event.target.value)}
              />
            </label>
            <div className="flex items-end gap-2 sm:col-span-2 lg:col-span-3 xl:col-span-6">
              <Button size="sm" variant="ghost" onClick={resetFilters}>
                Reset all filters
              </Button>
              <span className="text-[0.65rem] text-zinc-600">
                Base currency for converted totals: {app.primaryCurrency}
              </span>
            </div>
          </div>
        ) : null}

        {list.isError ? (
          <div className="p-5">
            <Alert tone="error" title="Could not load transactions">
              {errorMessage(list.error)}
            </Alert>
          </div>
        ) : list.isLoading ? (
          <div className="flex items-center justify-center gap-2 py-16 text-xs text-zinc-500">
            <Spinner className="h-4 w-4" /> Loading ledger…
          </div>
        ) : rows.length === 0 ? (
          <div className="p-5">
            <EmptyState
              icon={<Receipt className="h-5 w-5" />}
              title={hasFilters ? "No entries match these filters" : "No transactions yet"}
              description={
                hasFilters
                  ? "Adjust or reset the filters to see more of the ledger."
                  : "Record your first expense, income or transfer. Amounts are parsed and balanced in Rust."
              }
              action={
                hasFilters ? (
                  <Button size="sm" variant="secondary" onClick={resetFilters}>
                    Reset filters
                  </Button>
                ) : (
                  <Button size="sm" variant="primary" onClick={() => openComposer("expense")}>
                    <Plus className="h-3.5 w-3.5" /> Record entry
                  </Button>
                )
              }
            />
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[880px] border-collapse text-left">
              <thead>
                <tr className="border-b border-white/[0.055] text-[0.56rem] uppercase tracking-[0.16em] text-zinc-600">
                  <th scope="col" className="px-4 py-2.5 font-semibold">Date</th>
                  <th scope="col" className="px-2 py-2.5 font-semibold">Description</th>
                  <th scope="col" className="px-2 py-2.5 font-semibold">Account</th>
                  <th scope="col" className="px-2 py-2.5 font-semibold">Category</th>
                  <th scope="col" className="px-2 py-2.5 font-semibold">Status</th>
                  <th scope="col" className="px-2 py-2.5 text-right font-semibold">Amount</th>
                  <th scope="col" className="px-4 py-2.5 text-right font-semibold">Base</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/[0.04]">
                {rows.map((row) => (
                  <tr
                    key={row.id}
                    className="rawjan-table-row cursor-pointer"
                    onClick={() => setDetailId(row.id)}
                    tabIndex={0}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        setDetailId(row.id);
                      }
                    }}
                  >
                    <td className="whitespace-nowrap px-4 py-2.5 text-[0.7rem] text-zinc-500">
                      {formatDate(row.localDate)}
                    </td>
                    <td className="px-2 py-2.5">
                      <div className="flex items-center gap-2">
                        <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-white/[0.07] bg-white/[0.02] text-zinc-500">
                          {row.kind === "income" ? (
                            <ArrowDownLeft className="h-3 w-3 text-sage-400" />
                          ) : row.kind === "expense" || row.kind === "fee" ? (
                            <ArrowUpRight className="h-3 w-3 text-lily-300" />
                          ) : (
                            <ArrowLeftRight className="h-3 w-3 text-champagne-400" />
                          )}
                        </span>
                        <div className="min-w-0">
                          <div className="truncate text-[0.78rem] text-zinc-200">{row.description}</div>
                          <div className="mt-0.5 flex items-center gap-1.5 text-[0.6rem] text-zinc-600">
                            {row.payee ? <span className="truncate">{row.payee}</span> : null}
                            {row.isSplit ? <Badge tone="muted">Split</Badge> : null}
                            {row.reversalOfId ? <Badge tone="red">Reversal</Badge> : null}
                            {row.tags.slice(0, 2).map((tag) => (
                              <span key={tag} className="text-zinc-600">
                                #{tag}
                              </span>
                            ))}
                          </div>
                        </div>
                      </div>
                    </td>
                    <td className="px-2 py-2.5 text-[0.7rem] text-zinc-400">
                      <div className="truncate">{row.accountName}</div>
                      {row.counterAccountName ? (
                        <div className="truncate text-[0.6rem] text-zinc-600">→ {row.counterAccountName}</div>
                      ) : null}
                    </td>
                    <td className="max-w-[160px] px-2 py-2.5 text-[0.7rem] text-zinc-400">
                      <div className="truncate">{row.categoryName ?? "—"}</div>
                    </td>
                    <td className="px-2 py-2.5">
                      <div className="flex flex-wrap items-center gap-1">
                        <Badge tone={statusTone(row.status)}>
                          {STATUS_LABELS[row.status] ?? row.status}
                        </Badge>
                        {row.dataStatus !== "actual" ? (
                          <Badge tone="amber">{row.dataStatus}</Badge>
                        ) : null}
                        {row.status === "pending" || row.status === "draft" ? (
                          <CircleAlert className="h-3 w-3 text-amber-400/70" />
                        ) : null}
                      </div>
                    </td>
                    <td className={clsx("whitespace-nowrap px-2 py-2.5 text-right font-mono text-[0.74rem]", amountTone(row.kind))}>
                      {signedAmount(row.kind, row.amount)}
                      <div className="text-[0.56rem] uppercase tracking-wider text-zinc-600">
                        {row.accountCurrency}
                      </div>
                    </td>
                    <td className="whitespace-nowrap px-4 py-2.5 text-right font-mono text-[0.68rem] text-zinc-500">
                      {row.base}
                      <div className="text-[0.56rem] uppercase tracking-wider text-zinc-700">
                        {row.baseCurrency}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <div className="flex flex-wrap items-center justify-between gap-2 border-t border-white/[0.055] px-4 py-2.5">
          <div className="text-[0.65rem] text-zinc-600">
            {total === 0
              ? "No rows"
              : `Showing ${page * PAGE_SIZE + 1}–${Math.min(total, (page + 1) * PAGE_SIZE)} of ${total}`}
            {total > PAGE_SIZE ? ` · page ${page + 1} of ${pageCount}` : ""}
          </div>
          <div className="flex items-center gap-1.5">
            <Button
              size="sm"
              variant="ghost"
              disabled={page === 0 || list.isFetching}
              onClick={() => setPage((value) => Math.max(0, value - 1))}
            >
              <ChevronLeft className="h-3.5 w-3.5" /> Previous
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={page + 1 >= pageCount || list.isFetching}
              onClick={() => setPage((value) => Math.min(pageCount - 1, value + 1))}
            >
              Next <ChevronRight className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
      </Panel>

      <TransactionDetailModal id={detailId} onClose={() => setDetailId(null)} />
    </div>
  );
}
