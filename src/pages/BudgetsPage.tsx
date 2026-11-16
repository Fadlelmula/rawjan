import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import {
  AlertTriangle,
  Pause,
  Pencil,
  Play,
  Plus,
  Target,
  Trash2,
  X,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { Budget, BudgetInput } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { formatDate } from "@/lib/display";
import { useToast } from "@/components/Toast";
import {
  Alert,
  Badge,
  Button,
  EmptyState,
  Field,
  Modal,
  Panel,
  Spinner,
} from "@/components/ui";

const PERIOD_TYPES = [
  { value: "cycle_month", label: "Salary cycle" },
  { value: "calendar_month", label: "Calendar month" },
  { value: "weekly", label: "Weekly" },
  { value: "daily", label: "Daily" },
];

const METHODS = [
  { value: "custom", label: "Custom" },
  { value: "zero_based", label: "Zero-based" },
  { value: "envelope", label: "Envelope" },
  { value: "fifty_thirty_twenty", label: "50 / 30 / 20" },
];

const ROLLOVER = [
  { value: "none", label: "No rollover" },
  { value: "unspent", label: "Roll over unspent" },
  { value: "full", label: "Roll over full allocation" },
];

const PERIOD_LABEL: Record<string, string> = Object.fromEntries(
  PERIOD_TYPES.map((period) => [period.value, period.label]),
);
const METHOD_LABEL: Record<string, string> = Object.fromEntries(
  METHODS.map((method) => [method.value, method.label]),
);
const ROLLOVER_LABEL: Record<string, string> = Object.fromEntries(
  ROLLOVER.map((policy) => [policy.value, policy.label]),
);

interface AllocationDraft {
  categoryId: string;
  amount: string;
}

interface BudgetFormState {
  name: string;
  method: string;
  periodType: string;
  cycleStartDay: string;
  rolloverPolicy: string;
  currencyCode: string;
  startDate: string;
  allocations: AllocationDraft[];
}

function blankForm(primaryCurrency: string, today: string): BudgetFormState {
  return {
    name: "",
    method: "custom",
    periodType: "cycle_month",
    cycleStartDay: "28",
    rolloverPolicy: "none",
    currencyCode: primaryCurrency,
    startDate: today,
    allocations: [],
  };
}

function progressTone(percent: number, overBudget: boolean): string {
  if (overBudget || percent > 100) return "bg-lily-500";
  if (percent >= 90) return "bg-amber-400";
  if (percent >= 75) return "bg-champagne-400";
  return "bg-sage-500";
}

export function BudgetsPage() {
  const { app, currencies, categories } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Budget | null>(null);
  const [form, setForm] = useState<BudgetFormState>(() => blankForm(app.primaryCurrency, app.today));
  const [formError, setFormError] = useState<string | null>(null);

  const budgetsQuery = useQuery({
    queryKey: ["budgets", app.today],
    queryFn: () => api.listBudgets(app.today),
  });

  const expenseCategories = useMemo(
    () => categories.filter((category) => category.kind === "expense" && !category.isArchived),
    [categories],
  );

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["budgets"] }),
      queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
    ]);
  };

  const saveMutation = useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: BudgetInput }) =>
      api.saveBudget(id, input),
    onSuccess: async () => {
      await refresh();
      toast.success(editing ? "Budget updated" : "Budget created", "Spend is recalculated from posted ledger entries.");
      setModalOpen(false);
      setEditing(null);
    },
    onError: (error) => setFormError(errorMessage(error)),
  });

  const activeMutation = useMutation({
    mutationFn: ({ id, active }: { id: string; active: boolean }) =>
      api.setBudgetActive(id, active),
    onSuccess: async (_data, variables) => {
      await refresh();
      toast.success(variables.active ? "Budget resumed" : "Budget paused");
    },
    onError: (error) => toast.error("Budget not changed", errorMessage(error)),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.deleteBudget(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Budget deleted", "Transactions are untouched; only the spending plan was removed.");
    },
    onError: (error) => toast.error("Budget not deleted", errorMessage(error)),
  });

  const openCreate = () => {
    setEditing(null);
    setForm(blankForm(app.primaryCurrency, app.today));
    setFormError(null);
    setModalOpen(true);
  };

  const openEdit = (budget: Budget) => {
    setEditing(budget);
    setForm({
      name: budget.name,
      method: budget.method,
      periodType: budget.periodType,
      cycleStartDay: String(budget.cycleStartDay),
      rolloverPolicy: budget.rolloverPolicy,
      currencyCode: budget.currency,
      startDate: budget.startDate,
      allocations: budget.allocations.map((allocation) => ({
        categoryId: allocation.categoryId,
        amount: allocation.amount,
      })),
    });
    setFormError(null);
    setModalOpen(true);
  };

  const addAllocation = () => {
    const used = new Set(form.allocations.map((allocation) => allocation.categoryId));
    const next = expenseCategories.find((category) => !used.has(category.id));
    if (!next) {
      setFormError("Every expense category is already in this budget.");
      return;
    }
    setFormError(null);
    setForm({ ...form, allocations: [...form.allocations, { categoryId: next.id, amount: "" }] });
  };

  const submit = () => {
    setFormError(null);
    if (!form.name.trim()) {
      setFormError("Give the budget a name.");
      return;
    }
    const seen = new Set<string>();
    for (const allocation of form.allocations) {
      if (!allocation.categoryId) {
        setFormError("Choose a category for every allocation.");
        return;
      }
      if (seen.has(allocation.categoryId)) {
        setFormError("Each category can appear only once.");
        return;
      }
      seen.add(allocation.categoryId);
      if (!allocation.amount.trim()) {
        setFormError("Enter an amount for every allocation, or remove the row.");
        return;
      }
    }
    const input: BudgetInput = {
      name: form.name.trim(),
      method: form.method,
      periodType: form.periodType,
      cycleStartDay: form.periodType === "cycle_month" ? Number(form.cycleStartDay) || 28 : null,
      rolloverPolicy: form.rolloverPolicy,
      currencyCode: form.currencyCode,
      startDate: form.startDate || app.today,
      allocations: form.allocations.map((allocation) => ({
        categoryId: allocation.categoryId,
        amount: allocation.amount.trim(),
      })),
    };
    saveMutation.mutate({ id: editing?.id ?? null, input });
  };

  const budgets = budgetsQuery.data ?? [];

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Spending plan from posted entries
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Budgets</h2>
          <p className="mt-1 max-w-2xl text-xs leading-relaxed text-zinc-500">
            Allocations are yours to set. Actual spend is read from the ledger — posted and reconciled expenses,
            minus refunds. Pending and reversed rows never count.
          </p>
        </div>
        <Button size="sm" variant="primary" onClick={openCreate}>
          <Plus className="h-3.5 w-3.5" /> New budget
        </Button>
      </div>

      {budgetsQuery.isError ? (
        <Alert tone="error" title="Could not load budgets">
          {errorMessage(budgetsQuery.error)}
        </Alert>
      ) : null}

      {budgetsQuery.isLoading ? (
        <div className="flex items-center justify-center py-16 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : budgets.length === 0 ? (
        <EmptyState
          icon={<Target className="h-5 w-5" />}
          title="No budgets yet"
          description="Create a spending plan around your salary cycle. Start with the 28→27 cycle, then allocate expense categories. Nothing is pre-filled."
          action={
            <Button size="sm" variant="primary" onClick={openCreate}>
              <Plus className="h-3.5 w-3.5" /> Create first budget
            </Button>
          }
        />
      ) : (
        <div className="grid gap-3 xl:grid-cols-2">
          {budgets.map((budget) => (
            <BudgetCard
              key={budget.id}
              budget={budget}
              busy={activeMutation.isPending || deleteMutation.isPending}
              onEdit={() => openEdit(budget)}
              onToggle={() => activeMutation.mutate({ id: budget.id, active: !budget.isActive })}
              onDelete={() => {
                if (
                  window.confirm(
                    `Delete “${budget.name}”? The spending plan is removed, but your transactions and ledger history stay intact.`,
                  )
                ) {
                  deleteMutation.mutate(budget.id);
                }
              }}
            />
          ))}
        </div>
      )}

      <Modal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title={editing ? "Edit budget" : "New budget"}
        description="Period windows and spend are computed in Rust. Allocations are stored as exact minor units."
        width="lg"
        footer={
          <>
            <Button variant="ghost" onClick={() => setModalOpen(false)} disabled={saveMutation.isPending}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submit} loading={saveMutation.isPending}>
              {editing ? "Save changes" : "Create budget"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Budget name">
              <input
                className="rawjan-input"
                placeholder="e.g. Monthly essentials"
                value={form.name}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
                autoFocus
              />
            </Field>
            <Field label="Method">
              <select
                className="rawjan-input"
                value={form.method}
                onChange={(event) => setForm({ ...form, method: event.target.value })}
              >
                {METHODS.map((method) => (
                  <option key={method.value} value={method.value}>
                    {method.label}
                  </option>
                ))}
              </select>
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="Period">
              <select
                className="rawjan-input"
                value={form.periodType}
                onChange={(event) => setForm({ ...form, periodType: event.target.value })}
              >
                {PERIOD_TYPES.map((period) => (
                  <option key={period.value} value={period.value}>
                    {period.label}
                  </option>
                ))}
              </select>
            </Field>
            <Field
              label="Cycle start day"
              hint={form.periodType === "cycle_month" ? "Default 28 → runs 28th to 27th." : "Used only by salary cycles."}
            >
              <input
                type="number"
                min={1}
                max={31}
                className="rawjan-input"
                value={form.cycleStartDay}
                disabled={form.periodType !== "cycle_month"}
                onChange={(event) => setForm({ ...form, cycleStartDay: event.target.value })}
              />
            </Field>
            <Field label="Currency">
              <select
                className="rawjan-input"
                value={form.currencyCode}
                onChange={(event) => setForm({ ...form, currencyCode: event.target.value })}
              >
                {currencies.map((currency) => (
                  <option key={currency.code} value={currency.code}>
                    {currency.code} — {currency.name}
                  </option>
                ))}
              </select>
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Rollover policy">
              <select
                className="rawjan-input"
                value={form.rolloverPolicy}
                onChange={(event) => setForm({ ...form, rolloverPolicy: event.target.value })}
              >
                {ROLLOVER.map((policy) => (
                  <option key={policy.value} value={policy.value}>
                    {policy.label}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Start date">
              <input
                type="date"
                className="rawjan-input"
                value={form.startDate}
                onChange={(event) => setForm({ ...form, startDate: event.target.value })}
              />
            </Field>
          </div>

          <div>
            <div className="mb-2 flex items-center justify-between">
              <span className="rawjan-label mb-0">Category allocations</span>
              <Button size="sm" variant="secondary" onClick={addAllocation} disabled={expenseCategories.length === 0}>
                <Plus className="h-3 w-3" /> Add category
              </Button>
            </div>

            {expenseCategories.length === 0 ? (
              <Alert tone="warning">
                No expense categories exist yet. Create an expense category before allocating a budget.
              </Alert>
            ) : form.allocations.length === 0 ? (
              <div className="rounded-xl border border-dashed border-white/10 px-4 py-6 text-center text-[0.72rem] text-zinc-500">
                No allocations. A budget with zero allocations still tracks spend, but cannot show a target.
              </div>
            ) : (
              <div className="space-y-2">
                {form.allocations.map((allocation, index) => (
                  <div key={index} className="flex items-center gap-2">
                    <select
                      className="rawjan-input min-w-0 flex-1"
                      value={allocation.categoryId}
                      onChange={(event) => {
                        const next = [...form.allocations];
                        next[index] = { ...next[index], categoryId: event.target.value };
                        setForm({ ...form, allocations: next });
                      }}
                    >
                      {expenseCategories.map((category) => {
                        const usedElsewhere = form.allocations.some(
                          (row, rowIndex) => rowIndex !== index && row.categoryId === category.id,
                        );
                        return (
                          <option key={category.id} value={category.id} disabled={usedElsewhere}>
                            {category.name}
                          </option>
                        );
                      })}
                    </select>
                    <div className="relative w-36 shrink-0">
                      <input
                        className="rawjan-input pr-10 font-mono"
                        inputMode="decimal"
                        placeholder="0.00"
                        value={allocation.amount}
                        onChange={(event) => {
                          const next = [...form.allocations];
                          next[index] = { ...next[index], amount: event.target.value };
                          setForm({ ...form, allocations: next });
                        }}
                      />
                      <span className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-[0.6rem] text-zinc-600">
                        {form.currencyCode}
                      </span>
                    </div>
                    <button
                      type="button"
                      className="rounded-lg border border-transparent p-1.5 text-zinc-600 transition hover:border-white/10 hover:text-lily-300"
                      onClick={() =>
                        setForm({
                          ...form,
                          allocations: form.allocations.filter((_row, rowIndex) => rowIndex !== index),
                        })
                      }
                      aria-label="Remove allocation"
                    >
                      <X className="h-3.5 w-3.5" />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {form.rolloverPolicy !== "none" ? (
            <Alert tone="warning" title="Rollover is recorded, not applied silently">
              Rawjan stores this policy so it is visible and auditable. Carry-over is not yet folded into the
              displayed period totals.
            </Alert>
          ) : null}

          {formError ? (
            <Alert tone="error" title="Budget not saved">
              {formError}
            </Alert>
          ) : null}
        </div>
      </Modal>
    </div>
  );
}

function BudgetCard({
  budget,
  busy,
  onEdit,
  onToggle,
  onDelete,
}: {
  budget: Budget;
  busy: boolean;
  onEdit: () => void;
  onToggle: () => void;
  onDelete: () => void;
}) {
  const percent = Math.max(0, budget.percentUsed);
  const width = Math.min(100, percent);
  const over = budget.remainingMinor < 0;

  return (
    <Panel className={clsx("flex h-full flex-col p-4", !budget.isActive && "opacity-70")}>
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="truncate font-display text-[1.05rem] leading-tight text-zinc-100">{budget.name}</h3>
            {budget.isActive ? (
              <Badge tone="green">Active</Badge>
            ) : (
              <Badge tone="muted">Paused</Badge>
            )}
          </div>
          <div className="mt-0.5 truncate text-[0.62rem] text-zinc-600">
            {PERIOD_LABEL[budget.periodType] ?? budget.periodType}
            {budget.periodType === "cycle_month" ? ` · day ${budget.cycleStartDay}` : ""} ·{" "}
            {METHOD_LABEL[budget.method] ?? budget.method} · {ROLLOVER_LABEL[budget.rolloverPolicy] ?? budget.rolloverPolicy}
          </div>
          <div className="mt-0.5 text-[0.62rem] text-zinc-600">
            {formatDate(budget.periodStart)} – {formatDate(budget.periodEnd)}
          </div>
        </div>
        <Badge tone="muted">{budget.currency}</Badge>
      </div>

      {budget.fxUnavailable ? (
        <div className="mt-3 rounded-lg border border-amber-500/20 bg-amber-500/[0.06] px-3 py-2 text-[0.62rem] leading-relaxed text-amber-200/80">
          No exchange rate converts some spending into {budget.currency}. Those amounts are excluded
          from these totals instead of being counted unconverted.
        </div>
      ) : null}

      <div className="mt-4 grid grid-cols-3 gap-2">
        <div>
          <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Allocated</div>
          <div className="mt-0.5 font-display text-[0.95rem] text-zinc-200">{budget.allocated}</div>
        </div>
        <div>
          <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">
            Spent{budget.fxUnavailable ? " (partial)" : ""}
          </div>
          <div className={clsx("mt-0.5 font-display text-[0.95rem]", over ? "text-lily-200" : "text-zinc-200")}>
            {budget.spent}
          </div>
        </div>
        <div>
          <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Remaining</div>
          <div className={clsx("mt-0.5 font-display text-[0.95rem]", over ? "text-lily-200" : "text-sage-300")}>
            {budget.remaining}
          </div>
        </div>
      </div>

      <div className="mt-3">
        <div className="flex items-center justify-between text-[0.6rem] text-zinc-600">
          <span>
            {over ? "Over budget" : `${percent}% used`}
            {over ? ` · ${Math.abs(percent)}%` : ""}
          </span>
          {over ? <AlertTriangle className="h-3 w-3 text-lily-400" /> : null}
        </div>
        <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-white/[0.05]">
          <div
            className={clsx("h-full rounded-full transition-[width]", progressTone(percent, over))}
            style={{ width: `${width}%` }}
          />
        </div>
      </div>

      {budget.allocations.length > 0 ? (
        <div className="mt-4 space-y-2 border-t border-white/[0.05] pt-3">
          {budget.allocations.map((allocation) => {
            const allocationWidth = Math.min(
              100,
              Math.max(0, allocation.percentUsed ?? 0),
            );
            return (
              <div key={allocation.id}>
                <div className="flex items-center justify-between gap-2 text-[0.66rem]">
                  <span className="flex min-w-0 items-center gap-1.5 text-zinc-400">
                    {allocation.categoryColor ? (
                      <span
                        className="h-2 w-2 shrink-0 rounded-full"
                        style={{ backgroundColor: allocation.categoryColor }}
                      />
                    ) : null}
                    <span className="truncate">{allocation.categoryName}</span>
                  </span>
                  <span
                    className={clsx(
                      "shrink-0 font-mono",
                      allocation.overBudget ? "text-lily-300" : "text-zinc-500",
                    )}
                  >
                    {allocation.fxUnavailable ? "— / " : `${allocation.spent} / `}
                    {allocation.amount}
                  </span>
                </div>
                {allocation.fxUnavailable ? (
                  <div className="mt-0.5 text-[0.58rem] text-amber-200/70">
                    Spend could not be converted into {budget.currency} — no exchange rate.
                  </div>
                ) : null}
                <div className="mt-1 h-1 overflow-hidden rounded-full bg-white/[0.04]">
                  <div
                    className={clsx(
                      "h-full rounded-full",
                      progressTone(allocation.percentUsed ?? 0, allocation.overBudget ?? false),
                    )}
                    style={{ width: `${allocationWidth}%` }}
                  />
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="mt-4 border-t border-white/[0.05] pt-3 text-[0.66rem] text-zinc-600">
          No category allocations. Spend is not attributed to a target yet.
        </div>
      )}

      <div className="mt-auto flex items-center gap-1.5 pt-4">
        <Button size="sm" variant="secondary" onClick={onEdit}>
          <Pencil className="h-3 w-3" /> Edit
        </Button>
        <Button size="sm" variant="ghost" onClick={onToggle} disabled={busy}>
          {budget.isActive ? <Pause className="h-3 w-3" /> : <Play className="h-3 w-3" />}
          {budget.isActive ? "Pause" : "Resume"}
        </Button>
        <Button size="sm" variant="ghost" onClick={onDelete} disabled={busy} className="ml-auto text-zinc-500">
          <Trash2 className="h-3 w-3" />
        </Button>
      </div>
    </Panel>
  );
}
