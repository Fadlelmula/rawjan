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
  Landmark,
  Pencil,
  Plus,
  Trash2,
  Wallet,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type {
  Liability,
  LiabilityInput,
  LiabilityPayment,
  LiabilityPaymentInput,
} from "@/lib/api";
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

const TYPES = [
  { value: "credit_card", label: "Credit card" },
  { value: "loan", label: "Loan" },
  { value: "installment", label: "Installment" },
  { value: "money_borrowed", label: "Money borrowed" },
  { value: "money_lent", label: "Money lent (receivable)" },
  { value: "other", label: "Other" },
];

const STATUSES = [
  { value: "active", label: "Active" },
  { value: "paid", label: "Paid" },
  { value: "defaulted", label: "Defaulted" },
  { value: "archived", label: "Archived" },
];

const TYPE_LABEL: Record<string, string> = Object.fromEntries(
  TYPES.map((item) => [item.value, item.label]),
);
const STATUS_LABEL: Record<string, string> = Object.fromEntries(
  STATUSES.map((item) => [item.value, item.label]),
);

function statusTone(status: string): "green" | "red" | "champagne" | "muted" {
  if (status === "active") return "green";
  if (status === "defaulted") return "red";
  if (status === "paid") return "champagne";
  return "muted";
}

interface LiabilityFormState {
  name: string;
  liabilityType: string;
  accountId: string;
  principal: string;
  outstanding: string;
  currencyCode: string;
  interestRate: string;
  startDate: string;
  termMonths: string;
  payment: string;
  dueDay: string;
  firstPaymentDate: string;
  status: string;
  notes: string;
}

interface PaymentFormState {
  paidOn: string;
  amount: string;
  principal: string;
  interest: string;
  fee: string;
  note: string;
}

function blankForm(primaryCurrency: string, today: string): LiabilityFormState {
  return {
    name: "",
    liabilityType: "loan",
    accountId: "",
    principal: "",
    outstanding: "",
    currencyCode: primaryCurrency,
    interestRate: "",
    startDate: today,
    termMonths: "",
    payment: "",
    dueDay: "",
    firstPaymentDate: "",
    status: "active",
    notes: "",
  };
}

function progressTone(percent: number): string {
  if (percent >= 100) return "bg-champagne-400";
  if (percent >= 60) return "bg-sage-500";
  if (percent >= 25) return "bg-champagne-500";
  return "bg-lily-500";
}

export function LiabilitiesPage() {
  const { app, currencies, accounts } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();

  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<Liability | null>(null);
  const [form, setForm] = useState<LiabilityFormState>(() => blankForm(app.primaryCurrency, app.today));
  const [formError, setFormError] = useState<string | null>(null);

  const [paymentTarget, setPaymentTarget] = useState<Liability | null>(null);
  const [editingPayment, setEditingPayment] = useState<LiabilityPayment | null>(null);
  const [paymentForm, setPaymentForm] = useState<PaymentFormState>({
    paidOn: app.today,
    amount: "",
    principal: "",
    interest: "",
    fee: "",
    note: "",
  });
  const [paymentError, setPaymentError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const liabilityQuery = useQuery({
    queryKey: ["liabilities"],
    queryFn: () => api.listLiabilities(),
  });
  const summaryQuery = useQuery({
    queryKey: ["liabilities-summary"],
    queryFn: () => api.liabilitiesSummary(),
  });

  const accountOptions = useMemo(
    () => accounts.filter((account) => account.isActive && account.currency === form.currencyCode),
    [accounts, form.currencyCode],
  );

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["liabilities"] }),
      queryClient.invalidateQueries({ queryKey: ["liabilities-summary"] }),
      queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
    ]);
  };

  const saveMutation = useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: LiabilityInput }) =>
      api.saveLiability(id, input),
    onSuccess: async () => {
      await refresh();
      toast.success(
        editing ? "Liability updated" : "Liability created",
        "Net worth now uses this outstanding balance for the debt.",
      );
      setFormOpen(false);
      setEditing(null);
    },
    onError: (error) => setFormError(errorMessage(error)),
  });

  const paymentMutation = useMutation({
    mutationFn: ({
      liabilityId,
      id,
      input,
    }: {
      liabilityId: string;
      id: string | null;
      input: LiabilityPaymentInput;
    }) => api.saveLiabilityPayment(liabilityId, id, input),
    onSuccess: async () => {
      await refresh();
      toast.success(
        editingPayment ? "Payment updated" : "Payment recorded",
        "The outstanding balance was reduced by the principal portion. No ledger transaction was posted.",
      );
      setPaymentTarget(null);
      setEditingPayment(null);
    },
    onError: (error) => setPaymentError(errorMessage(error)),
  });

  const deletePaymentMutation = useMutation({
    mutationFn: (id: string) => api.deleteLiabilityPayment(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Payment removed", "The principal portion was restored to the outstanding balance.");
    },
    onError: (error) => toast.error("Payment not removed", errorMessage(error)),
  });

  const statusMutation = useMutation({
    mutationFn: ({ id, status }: { id: string; status: string }) => api.setLiabilityStatus(id, status),
    onSuccess: async (_data, variables) => {
      await refresh();
      toast.success(`Liability ${STATUS_LABEL[variables.status]?.toLowerCase() ?? "updated"}`);
    },
    onError: (error) => toast.error("Liability not changed", errorMessage(error)),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.deleteLiability(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Liability deleted", "Its payment history was removed. Ledger transactions are untouched.");
    },
    onError: (error) => toast.error("Liability not deleted", errorMessage(error)),
  });

  const openCreate = () => {
    setEditing(null);
    setForm(blankForm(app.primaryCurrency, app.today));
    setFormError(null);
    setFormOpen(true);
  };

  const openEdit = (liability: Liability) => {
    setEditing(liability);
    setForm({
      name: liability.name,
      liabilityType: liability.liabilityType,
      accountId: liability.accountId ?? "",
      principal: liability.principal,
      outstanding: liability.outstanding,
      currencyCode: liability.currency,
      interestRate: liability.interestRate,
      startDate: liability.startDate ?? "",
      termMonths: liability.termMonths === null ? "" : String(liability.termMonths),
      payment: liability.payment,
      dueDay: liability.dueDay === null ? "" : String(liability.dueDay),
      firstPaymentDate: liability.firstPaymentDate ?? "",
      status: liability.status,
      notes: liability.notes ?? "",
    });
    setFormError(null);
    setFormOpen(true);
  };

  const submit = () => {
    setFormError(null);
    if (!form.name.trim()) {
      setFormError("Give the liability a name.");
      return;
    }
    if (!form.principal.trim() || !form.outstanding.trim()) {
      setFormError("Enter both the original principal and the current outstanding balance.");
      return;
    }
    if (form.termMonths.trim()) {
      const term = Number(form.termMonths);
      if (!Number.isInteger(term) || term < 1 || term > 600) {
        setFormError("Term months must be a whole number between 1 and 600.");
        return;
      }
    }
    if (form.dueDay.trim()) {
      const day = Number(form.dueDay);
      if (!Number.isInteger(day) || day < 1 || day > 31) {
        setFormError("Due day must be a whole number between 1 and 31.");
        return;
      }
    }
    if (form.status === "paid" && !/^0(\.0+)?$/.test(form.outstanding.trim().replace(/,/g, ""))) {
      setFormError("Set the outstanding balance to zero before marking this liability paid.");
      return;
    }
    const input: LiabilityInput = {
      name: form.name.trim(),
      liabilityType: form.liabilityType,
      accountId: form.accountId || null,
      principal: form.principal.trim(),
      outstanding: form.outstanding.trim(),
      currencyCode: form.currencyCode,
      interestRate: form.interestRate.trim() || null,
      startDate: form.startDate || null,
      termMonths: form.termMonths.trim() ? Number(form.termMonths) : null,
      payment: form.payment.trim() || null,
      dueDay: form.dueDay.trim() ? Number(form.dueDay) : null,
      firstPaymentDate: form.firstPaymentDate || null,
      status: form.status,
      notes: form.notes.trim() || null,
    };
    saveMutation.mutate({ id: editing?.id ?? null, input });
  };

  const openPayment = (liability: Liability) => {
    setPaymentTarget(liability);
    setEditingPayment(null);
    setPaymentForm({
      paidOn: app.today,
      amount: "",
      principal: "",
      interest: "",
      fee: "",
      note: "",
    });
    setPaymentError(null);
  };

  const openEditPayment = (liability: Liability, payment: LiabilityPayment) => {
    setPaymentTarget(liability);
    setEditingPayment(payment);
    setPaymentForm({
      paidOn: payment.paidOn,
      amount: payment.amount,
      principal: payment.principal,
      interest: payment.interest,
      fee: payment.fee,
      note: payment.note ?? "",
    });
    setPaymentError(null);
  };

  const submitPayment = () => {
    if (!paymentTarget) return;
    setPaymentError(null);
    if (!paymentForm.amount.trim()) {
      setPaymentError("Enter the payment amount.");
      return;
    }
    paymentMutation.mutate({
      liabilityId: paymentTarget.id,
      id: editingPayment?.id ?? null,
      input: {
        paidOn: paymentForm.paidOn || app.today,
        amount: paymentForm.amount.trim(),
        principal: paymentForm.principal.trim() || null,
        interest: paymentForm.interest.trim() || null,
        fee: paymentForm.fee.trim() || null,
        note: paymentForm.note.trim() || null,
      },
    });
  };

  const liabilities = liabilityQuery.data ?? [];
  const summary = summaryQuery.data;

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            What is owed, stated plainly
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Liabilities</h2>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
            Outstanding balances are stored exactly as you state them. Recording a payment reduces the balance by its
            principal portion and keeps the interest and fee breakdown. Payments are records here: they do not post a
            ledger transaction or move a linked account's balance.
          </p>
        </div>
        <Button size="sm" variant="primary" onClick={openCreate}>
          <Plus className="h-3.5 w-3.5" /> New liability
        </Button>
      </div>

      {summaryQuery.isError ? (
        <Alert tone="error" title="Could not load the debt summary">
          {errorMessage(summaryQuery.error)}
        </Alert>
      ) : null}

      {summary && summary.currencies.length > 0 ? (
        <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
          {summary.currencies.map((currency) => (
            <Panel key={currency.currency} className="p-3.5">
              <div className="flex items-center justify-between">
                <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">
                  Debt · {currency.currency}
                </div>
                {currency.overdueCount > 0 ? (
                  <Badge tone="red">{currency.overdueCount} overdue</Badge>
                ) : (
                  <Badge tone="green">{currency.activeCount} active</Badge>
                )}
              </div>
              <div className="mt-2 grid grid-cols-2 gap-2">
                <div>
                  <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Owed</div>
                  <div className="mt-0.5 font-display text-[1.05rem] text-zinc-100">{currency.debt}</div>
                </div>
                <div>
                  <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Monthly</div>
                  <div className="mt-0.5 font-mono text-[0.85rem] text-zinc-400">{currency.monthlyPayment}</div>
                </div>
              </div>
              {currency.receivableMinor > 0 ? (
                <div className="mt-2 flex items-center justify-between border-t border-white/[0.05] pt-2 text-[0.64rem]">
                  <span className="text-zinc-600">Money lent to you</span>
                  <span className="font-mono text-champagne-300">{currency.receivable}</span>
                </div>
              ) : null}
            </Panel>
          ))}
        </div>
      ) : null}

      {liabilityQuery.isError ? (
        <Alert tone="error" title="Could not load liabilities">
          {errorMessage(liabilityQuery.error)}
        </Alert>
      ) : null}

      {liabilityQuery.isLoading ? (
        <div className="flex items-center justify-center py-16 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : liabilities.length === 0 ? (
        <EmptyState
          icon={<Landmark className="h-5 w-5" />}
          title="No liabilities recorded"
          description="Add a loan, credit card or money borrowed. Nothing is inferred from account balances and no debt is invented."
          action={
            <Button size="sm" variant="primary" onClick={openCreate}>
              <Plus className="h-3.5 w-3.5" /> Record first liability
            </Button>
          }
        />
      ) : (
        <div className="grid gap-3 xl:grid-cols-2">
          {liabilities.map((liability) => {
            const open = expanded[liability.id] ?? false;
            const active = liability.status === "active" || liability.status === "defaulted";
            const dueSoon =
              liability.daysUntilDue !== null && liability.daysUntilDue <= 7 && liability.outstandingMinor > 0;

            return (
              <Panel
                key={liability.id}
                className={clsx(
                  "flex h-full flex-col p-4",
                  liability.status === "archived" && "opacity-75",
                )}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <h3 className="truncate font-display text-[1.05rem] leading-tight text-zinc-100">
                        {liability.name}
                      </h3>
                      <Badge tone={statusTone(liability.status)}>
                        {STATUS_LABEL[liability.status] ?? liability.status}
                      </Badge>
                      {liability.isReceivable ? <Badge tone="champagne">Receivable</Badge> : null}
                      {liability.overdue ? <Badge tone="red">Past due</Badge> : null}
                    </div>
                    <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[0.62rem] text-zinc-600">
                      <span>{TYPE_LABEL[liability.liabilityType] ?? liability.liabilityType}</span>
                      {liability.accountName ? (
                        <span className="inline-flex items-center gap-1">
                          <Wallet className="h-3 w-3" /> {liability.accountName}
                        </span>
                      ) : null}
                      {liability.dueDay ? (
                        <span className="inline-flex items-center gap-1">
                          <CalendarDays className="h-3 w-3" /> Due day {liability.dueDay}
                        </span>
                      ) : null}
                    </div>
                  </div>
                  <Badge tone="muted">{liability.currency}</Badge>
                </div>

                <div className="mt-4">
                  <div className="flex items-end justify-between gap-2">
                    <div>
                      <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">
                        {liability.isReceivable ? "Still owed to you" : "Outstanding"}
                      </div>
                      <div className="mt-0.5 font-display text-[1.3rem] leading-none text-zinc-100">
                        {liability.outstanding}
                      </div>
                    </div>
                    <div className="text-right">
                      <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Original principal</div>
                      <div className="mt-0.5 font-mono text-[0.8rem] text-zinc-400">{liability.principal}</div>
                    </div>
                  </div>
                  <div className="mt-2 flex items-center justify-between text-[0.6rem] text-zinc-600">
                    <span>{liability.percentRepaid}% of principal repaid</span>
                    <span className={clsx(liability.outstandingMinor > 0 ? "text-zinc-500" : "text-sage-400")}>
                      {liability.outstandingMinor > 0 ? `${liability.principalRepaid} repaid` : "Balance cleared"}
                    </span>
                  </div>
                  <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-white/[0.05]">
                    <div
                      className={clsx("h-full rounded-full transition-[width]", progressTone(liability.percentRepaid))}
                      style={{ width: `${Math.min(100, Math.max(0, liability.percentRepaid))}%` }}
                    />
                  </div>
                </div>

                <div className="mt-3 grid grid-cols-2 gap-2 border-t border-white/[0.05] pt-3 text-[0.66rem] sm:grid-cols-4">
                  <div>
                    <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Rate</div>
                    <div className="mt-0.5 font-mono text-zinc-300">
                      {liability.interestRateBps > 0 ? `${liability.interestRate}%` : "—"}
                    </div>
                  </div>
                  <div>
                    <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Payment</div>
                    <div className="mt-0.5 font-mono text-zinc-300">
                      {liability.paymentMinor > 0 ? liability.payment : "—"}
                    </div>
                  </div>
                  <div>
                    <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">
                      {liability.overdue ? "Past due" : "Next due"}
                    </div>
                    <div
                      className={clsx(
                        "mt-0.5 font-mono",
                        liability.overdue ? "text-lily-300" : dueSoon ? "text-amber-300" : "text-zinc-300",
                      )}
                    >
                      {liability.nextDueDate ? formatDate(liability.nextDueDate) : "—"}
                    </div>
                  </div>
                  <div>
                    <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Est. payoff</div>
                    <div className="mt-0.5 font-mono text-zinc-300">
                      {liability.estimatedPayoffDate ? formatDate(liability.estimatedPayoffDate) : "—"}
                    </div>
                  </div>
                </div>

                {liability.estimatedPayoffMonths !== null && liability.outstandingMinor > 0 ? (
                  <div className="mt-2 text-[0.6rem] leading-relaxed text-zinc-600">
                    Payoff is an estimate assuming the rate and payment stay fixed for{" "}
                    {liability.estimatedPayoffMonths} month{liability.estimatedPayoffMonths === 1 ? "" : "s"}.
                  </div>
                ) : null}

                {liability.notes ? (
                  <p className="mt-2 line-clamp-2 text-[0.66rem] leading-relaxed text-zinc-500">{liability.notes}</p>
                ) : null}

                <div className="mt-auto flex flex-wrap items-center gap-1.5 pt-4">
                  {active ? (
                    <Button size="sm" variant="secondary" onClick={() => openPayment(liability)}>
                      <Plus className="h-3 w-3" /> Payment
                    </Button>
                  ) : null}
                  <Button size="sm" variant="ghost" onClick={() => openEdit(liability)}>
                    <Pencil className="h-3 w-3" /> Edit
                  </Button>
                  {liability.status === "active" ? (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={statusMutation.isPending || liability.outstandingMinor !== 0}
                      onClick={() => statusMutation.mutate({ id: liability.id, status: "paid" })}
                    >
                      <Check className="h-3 w-3" /> Mark paid
                    </Button>
                  ) : null}
                  {liability.status === "defaulted" ? (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={statusMutation.isPending}
                      onClick={() => statusMutation.mutate({ id: liability.id, status: "active" })}
                    >
                      <Check className="h-3 w-3" /> Reactivate
                    </Button>
                  ) : null}
                  {liability.status !== "archived" ? (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={statusMutation.isPending}
                      onClick={() => statusMutation.mutate({ id: liability.id, status: "archived" })}
                    >
                      <Archive className="h-3 w-3" /> Archive
                    </Button>
                  ) : (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={statusMutation.isPending}
                      onClick={() => statusMutation.mutate({ id: liability.id, status: "active" })}
                    >
                      <ArchiveRestore className="h-3 w-3" /> Restore
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="ghost"
                    className="ml-auto text-zinc-500"
                    aria-label={`Delete liability ${liability.name}`}
                    disabled={deleteMutation.isPending}
                    onClick={() => {
                      if (
                        window.confirm(
                          liability.payments.length > 0
                            ? `“${liability.name}” has ${liability.payments.length} recorded payment(s). Deleting is refused while history exists — archive the liability instead so the history is preserved.`
                            : `Delete “${liability.name}”? This cannot be undone.`,
                        )
                      ) {
                        deleteMutation.mutate(liability.id);
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
                    onClick={() => setExpanded({ ...expanded, [liability.id]: !open })}
                  >
                    <span>
                      {liability.payments.length === 0
                        ? "No payments recorded"
                        : `${liability.payments.length} payment${liability.payments.length === 1 ? "" : "s"}`}
                    </span>
                    {open ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
                  </button>

                  {open ? (
                    liability.payments.length === 0 ? (
                      <div className="mt-2 rounded-lg border border-dashed border-white/10 px-3 py-3 text-center text-[0.66rem] text-zinc-600">
                        Nothing recorded yet. Add a payment when one actually happens.
                      </div>
                    ) : (
                      <div className="mt-2 max-h-52 space-y-1.5 overflow-y-auto pr-1">
                        {liability.payments.map((entry) => (
                          <div
                            key={entry.id}
                            className="flex items-center justify-between gap-2 rounded-lg border border-white/[0.05] bg-white/[0.02] px-2.5 py-1.5"
                          >
                            <div className="min-w-0">
                              <div className="flex items-center gap-2 text-[0.68rem] text-zinc-300">
                                <span className="font-mono">{entry.amount}</span>
                                <span className="text-zinc-600">{formatDate(entry.paidOn)}</span>
                              </div>
                              <div className="truncate text-[0.6rem] text-zinc-600">
                                Principal {entry.principal}
                                {entry.interestMinor > 0 ? ` · Interest ${entry.interest}` : ""}
                                {entry.feeMinor > 0 ? ` · Fee ${entry.fee}` : ""}
                                {entry.note ? ` · ${entry.note}` : ""}
                              </div>
                            </div>
                            <div className="flex shrink-0 items-center gap-0.5">
                              <button
                                type="button"
                                className="rounded-md p-1 text-zinc-600 transition hover:text-zinc-200"
                                aria-label="Edit payment"
                                onClick={() => openEditPayment(liability, entry)}
                              >
                                <Pencil className="h-3 w-3" />
                              </button>
                              <button
                                type="button"
                                className="rounded-md p-1 text-zinc-600 transition hover:text-lily-300"
                                aria-label="Delete payment"
                                disabled={deletePaymentMutation.isPending}
                                onClick={() => {
                                  if (
                                    window.confirm(
                                      `Remove the ${entry.amount} payment from ${formatDate(entry.paidOn)}? The principal portion will be restored to the outstanding balance.`,
                                    )
                                  ) {
                                    deletePaymentMutation.mutate(entry.id);
                                  }
                                }}
                              >
                                <Trash2 className="h-3 w-3" />
                              </button>
                            </div>
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
        title={editing ? "Edit liability" : "New liability"}
        description="The outstanding balance is what net worth uses. Payments reduce it by the principal portion."
        width="lg"
        footer={
          <>
            <Button variant="ghost" onClick={() => setFormOpen(false)} disabled={saveMutation.isPending}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submit} loading={saveMutation.isPending}>
              {editing ? "Save changes" : "Create liability"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Name">
              <input
                className="rawjan-input"
                placeholder="e.g. Car loan"
                value={form.name}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
                autoFocus
              />
            </Field>
            <Field label="Type">
              <select
                className="rawjan-input"
                value={form.liabilityType}
                onChange={(event) => setForm({ ...form, liabilityType: event.target.value })}
              >
                {TYPES.map((item) => (
                  <option key={item.value} value={item.value}>
                    {item.label}
                  </option>
                ))}
              </select>
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="Currency" hint={editing && editing.payments.length > 0 ? "Locked: payments exist." : undefined}>
              <select
                className="rawjan-input"
                value={form.currencyCode}
                disabled={editing !== null && editing.payments.length > 0}
                onChange={(event) =>
                  setForm({ ...form, currencyCode: event.target.value, accountId: "" })
                }
              >
                {currencies.map((currency) => (
                  <option key={currency.code} value={currency.code}>
                    {currency.code} — {currency.name}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Original principal">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={form.principal}
                onChange={(event) => setForm({ ...form, principal: event.target.value })}
              />
            </Field>
            <Field label="Outstanding balance">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={form.outstanding}
                onChange={(event) => setForm({ ...form, outstanding: event.target.value })}
              />
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="Interest rate %" hint="Annual percentage, stored as basis points.">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="e.g. 5.25"
                value={form.interestRate}
                onChange={(event) => setForm({ ...form, interestRate: event.target.value })}
              />
            </Field>
            <Field label="Monthly payment" hint="Used only for the payoff estimate.">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={form.payment}
                onChange={(event) => setForm({ ...form, payment: event.target.value })}
              />
            </Field>
            <Field label="Due day" hint="Day of the month, 1–31.">
              <input
                className="rawjan-input font-mono"
                inputMode="numeric"
                placeholder="e.g. 28"
                value={form.dueDay}
                onChange={(event) => setForm({ ...form, dueDay: event.target.value })}
              />
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="Linked account" hint="Optional. Must use the same currency.">
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
            <Field label="Start date" hint="Optional.">
              <input
                type="date"
                className="rawjan-input"
                value={form.startDate}
                onChange={(event) => setForm({ ...form, startDate: event.target.value })}
              />
            </Field>
            <Field label="Term (months)" hint="Optional.">
              <input
                className="rawjan-input font-mono"
                inputMode="numeric"
                placeholder="e.g. 36"
                value={form.termMonths}
                onChange={(event) => setForm({ ...form, termMonths: event.target.value })}
              />
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="First payment date" hint="Optional.">
              <input
                type="date"
                className="rawjan-input"
                value={form.firstPaymentDate}
                onChange={(event) => setForm({ ...form, firstPaymentDate: event.target.value })}
              />
            </Field>
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
          </div>

          <Field label="Notes" hint="Optional context you want to keep.">
            <input
              className="rawjan-input"
              placeholder="e.g. 36-month vehicle loan"
              value={form.notes}
              onChange={(event) => setForm({ ...form, notes: event.target.value })}
            />
          </Field>

          {form.status === "paid" && form.outstanding.trim() !== "" && form.outstanding.trim() !== "0" ? (
            <Alert tone="warning" title="Paid requires a zero balance">
              Set the outstanding balance to 0 before marking this liability paid.
            </Alert>
          ) : null}

          {formError ? (
            <Alert tone="error" title="Liability not saved">
              {formError}
            </Alert>
          ) : null}
        </div>
      </Modal>

      <Modal
        open={paymentTarget !== null}
        onClose={() => {
          setPaymentTarget(null);
          setEditingPayment(null);
        }}
        title={
          editingPayment
            ? `Edit payment — ${paymentTarget?.name ?? ""}`
            : `Record payment — ${paymentTarget?.name ?? ""}`
        }
        description="Principal reduces the outstanding balance. Interest and fees are recorded for history but do not reduce it."
        footer={
          <>
            <Button
              variant="ghost"
              onClick={() => {
                setPaymentTarget(null);
                setEditingPayment(null);
              }}
              disabled={paymentMutation.isPending}
            >
              Cancel
            </Button>
            <Button variant="primary" onClick={submitPayment} loading={paymentMutation.isPending}>
              <Check className="h-3.5 w-3.5" /> {editingPayment ? "Save changes" : "Record payment"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          {paymentTarget ? (
            <div className="rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3">
              <div className="flex items-center justify-between text-[0.7rem]">
                <span className="text-zinc-500">Outstanding</span>
                <span className="font-mono text-zinc-300">{paymentTarget.outstanding}</span>
              </div>
              <div className="mt-1 flex items-center justify-between text-[0.7rem]">
                <span className="text-zinc-500">Monthly payment</span>
                <span className="font-mono text-zinc-300">{paymentTarget.payment}</span>
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
                  value={paymentForm.amount}
                  onChange={(event) => setPaymentForm({ ...paymentForm, amount: event.target.value })}
                  autoFocus
                />
                <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-[0.6rem] text-zinc-600">
                  {paymentTarget?.currency ?? ""}
                </span>
              </div>
            </Field>
            <Field label="Paid on">
              <input
                type="date"
                className="rawjan-input"
                value={paymentForm.paidOn}
                onChange={(event) => setPaymentForm({ ...paymentForm, paidOn: event.target.value })}
              />
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="Principal" hint="Leave blank to use the full amount.">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={paymentForm.principal}
                onChange={(event) => setPaymentForm({ ...paymentForm, principal: event.target.value })}
              />
            </Field>
            <Field label="Interest" hint="Optional.">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={paymentForm.interest}
                onChange={(event) => setPaymentForm({ ...paymentForm, interest: event.target.value })}
              />
            </Field>
            <Field label="Fee" hint="Optional.">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={paymentForm.fee}
                onChange={(event) => setPaymentForm({ ...paymentForm, fee: event.target.value })}
              />
            </Field>
          </div>

          <Field label="Note" hint="Optional.">
            <input
              className="rawjan-input"
              placeholder="e.g. November installment"
              value={paymentForm.note}
              onChange={(event) => setPaymentForm({ ...paymentForm, note: event.target.value })}
            />
          </Field>

          <Alert tone="info">
            Principal, interest and fee must add up to the payment amount. The outstanding balance drops by the principal
            portion only.
          </Alert>

          {paymentError ? (
            <Alert tone="error" title="Payment not saved">
              {paymentError}
            </Alert>
          ) : null}
        </div>
      </Modal>
    </div>
  );
}
