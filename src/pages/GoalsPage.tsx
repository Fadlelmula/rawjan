import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import {
  Archive,
  ArchiveRestore,
  CalendarDays,
  Check,
  ChevronDown,
  ChevronUp,
  Pause,
  Pencil,
  PiggyBank,
  Play,
  Plus,
  Trash2,
  Wallet,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { ContributionInput, Goal, GoalInput } from "@/lib/api";
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

const STATUSES = [
  { value: "active", label: "Active" },
  { value: "paused", label: "Paused" },
  { value: "achieved", label: "Achieved" },
  { value: "archived", label: "Archived" },
];

const PRIORITIES = [
  { value: 1, label: "1 — Essential" },
  { value: 2, label: "2 — High" },
  { value: 3, label: "3 — Normal" },
  { value: 4, label: "4 — Later" },
  { value: 5, label: "5 — Someday" },
];

const STATUS_LABEL: Record<string, string> = Object.fromEntries(
  STATUSES.map((status) => [status.value, status.label]),
);

function statusTone(status: string): "green" | "amber" | "champagne" | "muted" {
  if (status === "active") return "green";
  if (status === "achieved") return "champagne";
  if (status === "paused") return "amber";
  return "muted";
}

interface GoalFormState {
  name: string;
  targetAmount: string;
  currencyCode: string;
  targetDate: string;
  priority: string;
  accountId: string;
  monthlyContribution: string;
  startDate: string;
  status: string;
  notes: string;
}

function blankForm(primaryCurrency: string, today: string): GoalFormState {
  return {
    name: "",
    targetAmount: "",
    currencyCode: primaryCurrency,
    targetDate: "",
    priority: "3",
    accountId: "",
    monthlyContribution: "",
    startDate: today,
    status: "active",
    notes: "",
  };
}

function progressTone(percent: number): string {
  if (percent >= 100) return "bg-champagne-400";
  if (percent >= 75) return "bg-sage-500";
  if (percent >= 40) return "bg-champagne-500";
  return "bg-lily-500";
}

export function GoalsPage() {
  const { app, currencies, accounts } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();

  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<Goal | null>(null);
  const [form, setForm] = useState<GoalFormState>(() => blankForm(app.primaryCurrency, app.today));
  const [formError, setFormError] = useState<string | null>(null);

  const [contributeGoal, setContributeGoal] = useState<Goal | null>(null);
  const [contribution, setContribution] = useState({
    amount: "",
    contributedOn: app.today,
    accountId: "",
    note: "",
  });
  const [contributionError, setContributionError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const goalsQuery = useQuery({
    queryKey: ["goals", app.today],
    queryFn: () => api.listGoals(app.today),
  });

  const accountOptions = useMemo(
    () => accounts.filter((account) => account.isActive && !account.isLiability),
    [accounts],
  );

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["goals"] }),
      queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
    ]);
  };

  const saveMutation = useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: GoalInput }) => api.saveGoal(id, input),
    onSuccess: async () => {
      await refresh();
      toast.success(editing ? "Goal updated" : "Goal created", "Progress starts at zero until you record a contribution.");
      setFormOpen(false);
      setEditing(null);
    },
    onError: (error) => setFormError(errorMessage(error)),
  });

  const contributionMutation = useMutation({
    mutationFn: ({ goalId, input }: { goalId: string; input: ContributionInput }) =>
      api.addGoalContribution(goalId, input),
    onSuccess: async () => {
      await refresh();
      toast.success("Contribution recorded", "Progress now reflects this entry. The linked account balance is not changed.");
      setContributeGoal(null);
    },
    onError: (error) => setContributionError(errorMessage(error)),
  });

  const deleteContributionMutation = useMutation({
    mutationFn: (id: string) => api.deleteGoalContribution(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Contribution removed", "The goal's recorded progress was recalculated.");
    },
    onError: (error) => toast.error("Contribution not removed", errorMessage(error)),
  });

  const statusMutation = useMutation({
    mutationFn: ({ id, status }: { id: string; status: string }) => api.setGoalStatus(id, status),
    onSuccess: async (_data, variables) => {
      await refresh();
      toast.success(`Goal ${STATUS_LABEL[variables.status]?.toLowerCase() ?? "updated"}`);
    },
    onError: (error) => toast.error("Goal not changed", errorMessage(error)),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.deleteGoal(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Goal deleted", "Its contribution history was removed with it. Ledger transactions are untouched.");
    },
    onError: (error) => toast.error("Goal not deleted", errorMessage(error)),
  });

  const openCreate = () => {
    setEditing(null);
    setForm(blankForm(app.primaryCurrency, app.today));
    setFormError(null);
    setFormOpen(true);
  };

  const openEdit = (goal: Goal) => {
    setEditing(goal);
    setForm({
      name: goal.name,
      targetAmount: goal.targetAmount,
      currencyCode: goal.currency,
      targetDate: goal.targetDate ?? "",
      priority: String(goal.priority),
      accountId: goal.accountId ?? "",
      monthlyContribution: goal.monthlyContribution,
      startDate: goal.startDate,
      status: goal.status,
      notes: goal.notes ?? "",
    });
    setFormError(null);
    setFormOpen(true);
  };

  const submit = () => {
    setFormError(null);
    if (!form.name.trim()) {
      setFormError("Give the goal a name.");
      return;
    }
    if (!form.targetAmount.trim()) {
      setFormError("Enter a target amount.");
      return;
    }
    const input: GoalInput = {
      name: form.name.trim(),
      targetAmount: form.targetAmount.trim(),
      currencyCode: form.currencyCode,
      targetDate: form.targetDate || null,
      priority: Number(form.priority) || 3,
      accountId: form.accountId || null,
      monthlyContribution: form.monthlyContribution.trim() || null,
      startDate: form.startDate || app.today,
      status: form.status,
      notes: form.notes.trim() || null,
    };
    saveMutation.mutate({ id: editing?.id ?? null, input });
  };

  const openContribute = (goal: Goal) => {
    setContributeGoal(goal);
    setContribution({ amount: "", contributedOn: app.today, accountId: goal.accountId ?? "", note: "" });
    setContributionError(null);
  };

  const submitContribution = () => {
    if (!contributeGoal) return;
    setContributionError(null);
    if (!contribution.amount.trim()) {
      setContributionError("Enter a contribution amount.");
      return;
    }
    contributionMutation.mutate({
      goalId: contributeGoal.id,
      input: {
        amount: contribution.amount.trim(),
        contributedOn: contribution.contributedOn || app.today,
        accountId: contribution.accountId || null,
        note: contribution.note.trim() || null,
      },
    });
  };

  const goals = goalsQuery.data ?? [];
  const activeCount = goals.filter((goal) => goal.status === "active").length;
  const achievedCount = goals.filter((goal) => goal.status === "achieved").length;
  const pausedCount = goals.filter((goal) => goal.status === "paused").length;
  const archivedCount = goals.filter((goal) => goal.status === "archived").length;

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Set aside, honestly
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Savings goals</h2>
          <p className="mt-1 max-w-2xl text-xs leading-relaxed text-zinc-500">
            Progress comes only from contributions you record here. Rawjan never reads a goal's progress from a bank
            balance, and recording a contribution does not move money in the ledger.
          </p>
        </div>
        <Button size="sm" variant="primary" onClick={openCreate}>
          <Plus className="h-3.5 w-3.5" /> New goal
        </Button>
      </div>

      {goals.length > 0 ? (
        <div className="grid gap-2 sm:grid-cols-4">
          <SummaryTile label="Active" value={activeCount} />
          <SummaryTile label="Paused" value={pausedCount} />
          <SummaryTile label="Achieved" value={achievedCount} />
          <SummaryTile label="Archived" value={archivedCount} />
        </div>
      ) : null}

      {goalsQuery.isError ? (
        <Alert tone="error" title="Could not load goals">
          {errorMessage(goalsQuery.error)}
        </Alert>
      ) : null}

      {goalsQuery.isLoading ? (
        <div className="flex items-center justify-center py-16 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : goals.length === 0 ? (
        <EmptyState
          icon={<PiggyBank className="h-5 w-5" />}
          title="No savings goals yet"
          description="Create a target, then record contributions as they happen. Nothing is pre-filled and no goal is invented for you."
          action={
            <Button size="sm" variant="primary" onClick={openCreate}>
              <Plus className="h-3.5 w-3.5" /> Create first goal
            </Button>
          }
        />
      ) : (
        <div className="grid gap-3 xl:grid-cols-2">
          {goals.map((goal) => {
            const percent = Math.max(0, goal.percentUsed);
            const width = Math.min(100, percent);
            const open = expanded[goal.id] ?? false;
            const overdue =
              goal.targetDate !== null &&
              goal.monthsRemaining === 0 &&
              goal.remainingMinor > 0 &&
              goal.status !== "achieved" &&
              goal.status !== "archived";

            return (
              <Panel
                key={goal.id}
                className={clsx(
                  "flex h-full flex-col p-4",
                  (goal.status === "archived" || goal.status === "achieved") && "opacity-80",
                )}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <h3 className="truncate font-display text-[1.05rem] leading-tight text-zinc-100">
                        {goal.name}
                      </h3>
                      <Badge tone={statusTone(goal.status)}>{STATUS_LABEL[goal.status] ?? goal.status}</Badge>
                      {goal.achieved && goal.status !== "achieved" ? (
                        <Badge tone="champagne">Fully funded</Badge>
                      ) : null}
                    </div>
                    <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[0.62rem] text-zinc-600">
                      <span>Priority {goal.priority}</span>
                      {goal.targetDate ? (
                        <span className="inline-flex items-center gap-1">
                          <CalendarDays className="h-3 w-3" /> {formatDate(goal.targetDate)}
                        </span>
                      ) : (
                        <span>No target date</span>
                      )}
                      {goal.accountName ? (
                        <span className="inline-flex items-center gap-1">
                          <Wallet className="h-3 w-3" /> {goal.accountName}
                        </span>
                      ) : null}
                    </div>
                  </div>
                  <Badge tone="muted">{goal.currency}</Badge>
                </div>

                <div className="mt-4">
                  <div className="flex items-end justify-between gap-2">
                    <div>
                      <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Contributed</div>
                      <div className="mt-0.5 font-display text-[1.3rem] leading-none text-zinc-100">
                        {goal.contributed}
                      </div>
                    </div>
                    <div className="text-right">
                      <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Target</div>
                      <div className="mt-0.5 font-mono text-[0.8rem] text-zinc-400">{goal.targetAmount}</div>
                    </div>
                  </div>
                  <div className="mt-2 flex items-center justify-between text-[0.6rem] text-zinc-600">
                    <span>{percent}% funded</span>
                    <span className={clsx(goal.remainingMinor > 0 ? "text-zinc-500" : "text-sage-400")}>
                      {goal.remainingMinor > 0 ? `${goal.remaining} to go` : "Target reached"}
                    </span>
                  </div>
                  <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-white/[0.05]">
                    <div
                      className={clsx("h-full rounded-full transition-[width]", progressTone(percent))}
                      style={{ width: `${width}%` }}
                    />
                  </div>
                </div>

                <div className="mt-3 grid grid-cols-2 gap-2 border-t border-white/[0.05] pt-3 text-[0.66rem]">
                  <div>
                    <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Planned monthly</div>
                    <div className="mt-0.5 font-mono text-zinc-300">{goal.monthlyContribution}</div>
                  </div>
                  <div>
                    <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">
                      {goal.remainingMinor <= 0 ? "Status" : overdue ? "Target date passed" : "Est. monthly needed"}
                    </div>
                    <div
                      className={clsx(
                        "mt-0.5 font-mono",
                        overdue ? "text-lily-300" : "text-zinc-300",
                      )}
                    >
                      {goal.remainingMinor <= 0
                        ? "Complete"
                        : overdue
                          ? `${goal.remaining} still due`
                          : (goal.requiredMonthly ?? "Set a target date")}
                    </div>
                  </div>
                </div>

                {goal.notes ? (
                  <p className="mt-2 line-clamp-2 text-[0.66rem] leading-relaxed text-zinc-500">{goal.notes}</p>
                ) : null}

                <div className="mt-auto flex flex-wrap items-center gap-1.5 pt-4">
                  {goal.status !== "archived" ? (
                    <Button size="sm" variant="secondary" onClick={() => openContribute(goal)}>
                      <Plus className="h-3 w-3" /> Add contribution
                    </Button>
                  ) : null}
                  <Button size="sm" variant="ghost" onClick={() => openEdit(goal)}>
                    <Pencil className="h-3 w-3" /> Edit
                  </Button>
                  {goal.status === "active" ? (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={statusMutation.isPending}
                      onClick={() => statusMutation.mutate({ id: goal.id, status: "paused" })}
                    >
                      <Pause className="h-3 w-3" /> Pause
                    </Button>
                  ) : null}
                  {goal.status === "paused" ? (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={statusMutation.isPending}
                      onClick={() => statusMutation.mutate({ id: goal.id, status: "active" })}
                    >
                      <Play className="h-3 w-3" /> Resume
                    </Button>
                  ) : null}
                  {goal.status === "archived" ? (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={statusMutation.isPending}
                      onClick={() => statusMutation.mutate({ id: goal.id, status: "active" })}
                    >
                      <ArchiveRestore className="h-3 w-3" /> Restore
                    </Button>
                  ) : null}
                  {goal.status !== "archived" ? (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={statusMutation.isPending}
                      onClick={() => statusMutation.mutate({ id: goal.id, status: "archived" })}
                    >
                      <Archive className="h-3 w-3" /> Archive
                    </Button>
                  ) : null}
                  <Button
                    size="sm"
                    variant="ghost"
                    className="ml-auto text-zinc-500"
                    aria-label={`Delete goal ${goal.name}`}
                    disabled={deleteMutation.isPending}
                    onClick={() => {
                      if (
                        window.confirm(
                          goal.contributions.length > 0
                            ? `“${goal.name}” has ${goal.contributions.length} recorded contribution(s). Deleting is refused while history exists — archive the goal instead so the history is preserved.`
                            : `Delete “${goal.name}”? This cannot be undone.`,
                        )
                      ) {
                        deleteMutation.mutate(goal.id);
                      }
                    }}
                  >
                    <Trash2 className="h-3 w-3" />
                  </Button>
                </div>

                <div className="mt-3 border-t border-white/[0.05] pt-2">
                  <button
                    type="button"
                    className="flex w-full items-center justify-between text-[0.62rem] text-zinc-500 transition hover:text-zinc-300"
                    onClick={() => setExpanded({ ...expanded, [goal.id]: !open })}
                  >
                    <span>
                      {goal.contributions.length === 0
                        ? "No contributions recorded"
                        : `${goal.contributions.length} contribution${goal.contributions.length === 1 ? "" : "s"}`}
                    </span>
                    {open ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
                  </button>

                  {open ? (
                    goal.contributions.length === 0 ? (
                      <div className="mt-2 rounded-lg border border-dashed border-white/10 px-3 py-3 text-center text-[0.66rem] text-zinc-600">
                        Nothing recorded yet. Add a contribution when you actually set money aside.
                      </div>
                    ) : (
                      <div className="mt-2 max-h-48 space-y-1.5 overflow-y-auto pr-1">
                        {goal.contributions.map((entry) => (
                          <div
                            key={entry.id}
                            className="flex items-center justify-between gap-2 rounded-lg border border-white/[0.05] bg-white/[0.02] px-2.5 py-1.5"
                          >
                            <div className="min-w-0">
                              <div className="flex items-center gap-2 text-[0.68rem] text-zinc-300">
                                <span className="font-mono">{entry.amount}</span>
                                <span className="text-zinc-600">{formatDate(entry.contributedOn)}</span>
                              </div>
                              <div className="truncate text-[0.6rem] text-zinc-600">
                                {entry.accountName ?? "No account linked"}
                                {entry.note ? ` · ${entry.note}` : ""}
                              </div>
                            </div>
                            <button
                              type="button"
                              className="rounded-md p-1 text-zinc-600 transition hover:text-lily-300"
                              aria-label="Delete contribution"
                              disabled={deleteContributionMutation.isPending}
                              onClick={() => {
                                if (window.confirm(`Remove the ${entry.amount} contribution from ${formatDate(entry.contributedOn)}?`)) {
                                  deleteContributionMutation.mutate(entry.id);
                                }
                              }}
                            >
                              <Trash2 className="h-3 w-3" />
                            </button>
                          </div>
                        ))}
                      </div>
                    )
                  ) : null}
                </div>
              </Panel>
            );
          })}
        </div>
      )}

      <Modal
        open={formOpen}
        onClose={() => setFormOpen(false)}
        title={editing ? "Edit goal" : "New savings goal"}
        description="Amounts are stored as exact minor units in the goal's own currency. Progress is calculated from recorded contributions."
        width="lg"
        footer={
          <>
            <Button variant="ghost" onClick={() => setFormOpen(false)} disabled={saveMutation.isPending}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submit} loading={saveMutation.isPending}>
              {editing ? "Save changes" : "Create goal"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Goal name">
              <input
                className="rawjan-input"
                placeholder="e.g. Emergency fund"
                value={form.name}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
                autoFocus
              />
            </Field>
            <Field label="Target amount">
              <div className="relative">
                <input
                  className="rawjan-input pr-12 font-mono"
                  inputMode="decimal"
                  placeholder="0.00"
                  value={form.targetAmount}
                  onChange={(event) => setForm({ ...form, targetAmount: event.target.value })}
                />
                <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-[0.6rem] text-zinc-600">
                  {form.currencyCode}
                </span>
              </div>
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="Currency">
              <select
                className="rawjan-input"
                value={form.currencyCode}
                disabled={editing !== null && editing.contributions.length > 0}
                onChange={(event) => setForm({ ...form, currencyCode: event.target.value })}
              >
                {currencies.map((currency) => (
                  <option key={currency.code} value={currency.code}>
                    {currency.code} — {currency.name}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Target date" hint="Optional. Used only for the monthly estimate.">
              <input
                type="date"
                className="rawjan-input"
                value={form.targetDate}
                onChange={(event) => setForm({ ...form, targetDate: event.target.value })}
              />
            </Field>
            <Field label="Priority">
              <select
                className="rawjan-input"
                value={form.priority}
                onChange={(event) => setForm({ ...form, priority: event.target.value })}
              >
                {PRIORITIES.map((priority) => (
                  <option key={priority.value} value={priority.value}>
                    {priority.label}
                  </option>
                ))}
              </select>
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="Linked account" hint="Optional. Purely a reference; balances are not moved.">
              <select
                className="rawjan-input"
                value={form.accountId}
                onChange={(event) => setForm({ ...form, accountId: event.target.value })}
              >
                <option value="">No linked account</option>
                {accountOptions.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name} · {account.currency}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Planned monthly contribution" hint="Your intention, not a ledger instruction.">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={form.monthlyContribution}
                onChange={(event) => setForm({ ...form, monthlyContribution: event.target.value })}
              />
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

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Status">
              <select
                className="rawjan-input"
                value={form.status}
                onChange={(event) => setForm({ ...form, status: event.target.value })}
              >
                {STATUSES.map((status) => (
                  <option key={status.value} value={status.value}>
                    {status.label}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Notes" hint="Optional context you want to keep with the goal.">
              <input
                className="rawjan-input"
                placeholder="Why this goal matters"
                value={form.notes}
                onChange={(event) => setForm({ ...form, notes: event.target.value })}
              />
            </Field>
          </div>

          {editing && editing.contributions.length > 0 && form.currencyCode !== editing.currency ? (
            <Alert tone="warning" title="Currency is locked">
              This goal already has contributions in {editing.currency}. Create a new goal instead of changing the
              currency, so past contributions are never reinterpreted.
            </Alert>
          ) : null}

          {formError ? (
            <Alert tone="error" title="Goal not saved">
              {formError}
            </Alert>
          ) : null}
        </div>
      </Modal>

      <Modal
        open={contributeGoal !== null}
        onClose={() => setContributeGoal(null)}
        title={contributeGoal ? `Add contribution — ${contributeGoal.name}` : "Add contribution"}
        description="This records progress toward the goal. It does not post a transaction or move an account balance."
        footer={
          <>
            <Button variant="ghost" onClick={() => setContributeGoal(null)} disabled={contributionMutation.isPending}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submitContribution} loading={contributionMutation.isPending}>
              <Check className="h-3.5 w-3.5" /> Record contribution
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          {contributeGoal ? (
            <div className="rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3">
              <div className="flex items-center justify-between text-[0.7rem]">
                <span className="text-zinc-500">Already recorded</span>
                <span className="font-mono text-zinc-300">{contributeGoal.contributed}</span>
              </div>
              <div className="mt-1 flex items-center justify-between text-[0.7rem]">
                <span className="text-zinc-500">Remaining to target</span>
                <span className="font-mono text-zinc-300">{contributeGoal.remaining}</span>
              </div>
            </div>
          ) : null}

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Amount">
              <div className="relative">
                <input
                  className="rawjan-input pr-12 font-mono"
                  inputMode="decimal"
                  placeholder="0.00"
                  value={contribution.amount}
                  onChange={(event) => setContribution({ ...contribution, amount: event.target.value })}
                  autoFocus
                />
                <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-[0.6rem] text-zinc-600">
                  {contributeGoal?.currency ?? ""}
                </span>
              </div>
            </Field>
            <Field label="Date">
              <input
                type="date"
                className="rawjan-input"
                value={contribution.contributedOn}
                onChange={(event) => setContribution({ ...contribution, contributedOn: event.target.value })}
              />
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Account reference" hint="Optional. Does not post to the ledger.">
              <select
                className="rawjan-input"
                value={contribution.accountId}
                onChange={(event) => setContribution({ ...contribution, accountId: event.target.value })}
              >
                <option value="">No account</option>
                {accountOptions.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name} · {account.currency}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Note" hint="Optional.">
              <input
                className="rawjan-input"
                placeholder="e.g. October transfer"
                value={contribution.note}
                onChange={(event) => setContribution({ ...contribution, note: event.target.value })}
              />
            </Field>
          </div>

          {contributionError ? (
            <Alert tone="error" title="Contribution not recorded">
              {contributionError}
            </Alert>
          ) : null}
        </div>
      </Modal>
    </div>
  );
}

function SummaryTile({ label, value }: { label: string; value: number }) {
  return (
    <Panel className="px-3 py-2.5">
      <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">{label}</div>
      <div className="mt-0.5 font-display text-[1.15rem] leading-none text-zinc-200">{value}</div>
    </Panel>
  );
}
