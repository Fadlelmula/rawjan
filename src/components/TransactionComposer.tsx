import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowRight, CircleAlert } from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { TransactionInput } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { todayInputValue } from "@/lib/display";
import { useToast } from "@/components/Toast";
import { Alert, Button, Field, Modal } from "@/components/ui";

const KIND_OPTIONS = [
  { value: "expense", label: "Expense" },
  { value: "income", label: "Income" },
  { value: "transfer", label: "Transfer" },
  { value: "refund", label: "Refund" },
  { value: "fee", label: "Fee" },
];

const STATUS_OPTIONS = [
  { value: "posted", label: "Posted — affects balances and reports" },
  { value: "pending", label: "Pending — affects balances, excluded from reports" },
  { value: "draft", label: "Draft — excluded from balances and reports" },
];

export function TransactionComposer({
  open,
  initialKind,
  onClose,
}: {
  open: boolean;
  initialKind: string;
  onClose: () => void;
}) {
  const { app, accounts, categories } = useBootstrap();
  const toast = useToast();
  const queryClient = useQueryClient();
  const activeAccounts = useMemo(() => accounts.filter((account) => account.isActive), [accounts]);

  const [kind, setKind] = useState(initialKind);
  const [status, setStatus] = useState("posted");
  const [accountId, setAccountId] = useState("");
  const [counterAccountId, setCounterAccountId] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const [incomeSourceId, setIncomeSourceId] = useState("");
  const [amount, setAmount] = useState("");
  const [counterAmount, setCounterAmount] = useState("");
  const [fxRate, setFxRate] = useState("");
  const [date, setDate] = useState(todayInputValue());
  const [description, setDescription] = useState("");
  const [payee, setPayee] = useState("");
  const [notes, setNotes] = useState("");
  const [tags, setTags] = useState("");
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setKind(initialKind);
    setStatus("posted");
    setAccountId(activeAccounts[0]?.id ?? "");
    setCounterAccountId("");
    setCategoryId("");
    setIncomeSourceId("");
    setAmount("");
    setCounterAmount("");
    setFxRate("");
    setDate(todayInputValue());
    setDescription("");
    setPayee("");
    setNotes("");
    setTags("");
    setFormError(null);
    // Only reset when the dialog is opened.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, initialKind]);

  const selectedAccount = activeAccounts.find((account) => account.id === accountId);
  const selectedCounter = activeAccounts.find((account) => account.id === counterAccountId);
  const isTransfer = kind === "transfer";
  const isIncome = kind === "income";
  const needsCounter = isTransfer;
  const crossCurrency =
    isTransfer && selectedAccount && selectedCounter && selectedAccount.currency !== selectedCounter.currency;
  const foreignAccount = selectedAccount && selectedAccount.currency !== app.primaryCurrency;

  const categoryOptions = useMemo(() => {
    const wanted = isIncome ? "income" : "expense";
    return categories
      .filter((category) => category.kind === wanted && !category.isArchived)
      .sort((a, b) => {
        if (a.parentId && !b.parentId) return 1;
        if (!a.parentId && b.parentId) return -1;
        return a.name.localeCompare(b.name);
      });
  }, [categories, isIncome]);

  useEffect(() => {
    setCategoryId((current) => {
      if (current && categoryOptions.some((category) => category.id === current)) return current;
      return categoryOptions[0]?.id ?? "";
    });
  }, [categoryOptions]);

  const incomeSourcesQuery = useQuery({
    queryKey: ["income-sources"],
    queryFn: () => api.listIncomeSources(),
    enabled: open && isIncome,
  });
  const incomeSources = useMemo(
    () => (incomeSourcesQuery.data ?? []).filter((source) => source.isActive),
    [incomeSourcesQuery.data],
  );

  const selectIncomeSource = (nextId: string) => {
    setIncomeSourceId(nextId);
    const source = incomeSources.find((candidate) => candidate.id === nextId);
    if (!source) return;
    if (source.categoryId && categoryOptions.some((category) => category.id === source.categoryId)) {
      setCategoryId(source.categoryId);
    }
    if (source.accountId && activeAccounts.some((account) => account.id === source.accountId)) {
      setAccountId(source.accountId);
    }
  };

  const mutation = useMutation({
    mutationFn: (input: TransactionInput) => api.createTransaction(input),
    onSuccess: async (result) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
        queryClient.invalidateQueries({ queryKey: ["transactions"] }),
        queryClient.invalidateQueries({ queryKey: ["account"] }),
        queryClient.invalidateQueries({ queryKey: ["income"] }),
      ]);
      const detail = result.possibleDuplicateId
        ? "Saved, but a possible duplicate was detected. Review the ledger for repeats."
        : `${selectedAccount?.currency ?? app.primaryCurrency} ${amount} recorded.`;
      toast.success(result.possibleDuplicateId ? "Entry recorded with a warning" : "Entry recorded", detail);
      onClose();
    },
    onError: (error) => setFormError(errorMessage(error)),
  });

  const submit = () => {
    setFormError(null);
    if (!accountId) {
      setFormError("Create an account before recording a transaction.");
      return;
    }
    if (!description.trim()) {
      setFormError("Add a short description.");
      return;
    }
    if (!amount.trim()) {
      setFormError("Enter an amount.");
      return;
    }
    if (isTransfer && !counterAccountId) {
      setFormError("Select the destination account for the transfer.");
      return;
    }
    if (crossCurrency && !counterAmount.trim()) {
      setFormError(`Enter the amount received in ${selectedCounter?.currency}.`);
      return;
    }
    if (!isTransfer && !categoryId) {
      setFormError("Select a category.");
      return;
    }

    mutation.mutate({
      kind,
      status,
      description: description.trim(),
      payee: payee.trim() || null,
      notes: notes.trim() || null,
      date,
      accountId,
      counterAccountId: needsCounter ? counterAccountId : null,
      categoryId: isTransfer ? null : categoryId || null,
      incomeSourceId: isIncome ? incomeSourceId || null : null,
      amount: amount.trim(),
      currency: selectedAccount?.currency ?? app.primaryCurrency,
      counterAmount: crossCurrency ? counterAmount.trim() : null,
      fxRate: foreignAccount && fxRate.trim() ? fxRate.trim() : null,
      tags: tags
        .split(",")
        .map((tag) => tag.trim())
        .filter(Boolean),
    });
  };

  const kindNoun = KIND_OPTIONS.find((option) => option.value === kind)?.label ?? "Entry";

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={`New ${kindNoun.toLowerCase()}`}
      description="Amounts are stored as integer minor units in Rust. The interface never performs financial arithmetic."
      width="lg"
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={mutation.isPending}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submit} loading={mutation.isPending}>
            Record {kindNoun.toLowerCase()}
          </Button>
        </>
      }
    >
      {activeAccounts.length === 0 ? (
        <Alert tone="warning" title="No accounts yet">
          Create a bank, cash or wallet account first. Every transaction must belong to an account.
        </Alert>
      ) : (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            {KIND_OPTIONS.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => setKind(option.value)}
                className={
                  kind === option.value
                    ? "rounded-xl border border-lily-500/55 bg-lily-600/15 px-3 py-2 text-left text-xs font-medium text-lily-100"
                    : "rounded-xl border border-white/[0.07] bg-white/[0.02] px-3 py-2 text-left text-xs font-medium text-zinc-400 transition hover:border-white/15 hover:text-zinc-200"
                }
              >
                {option.label}
              </button>
            ))}
          </div>

          <div className="grid gap-3 sm:grid-cols-[1.4fr_1fr]">
            <Field label="Amount">
              <div className="relative">
                <input
                  className="rawjan-input pr-16 font-mono text-base tracking-wide"
                  inputMode="decimal"
                  placeholder="0.00"
                  value={amount}
                  onChange={(event) => setAmount(event.target.value)}
                  autoFocus
                />
                <span className="absolute right-3 top-1/2 -translate-y-1/2 text-[0.68rem] font-medium tracking-wider text-zinc-500">
                  {selectedAccount?.currency ?? app.primaryCurrency}
                </span>
              </div>
            </Field>
            <Field label="Date">
              <input
                type="date"
                className="rawjan-input"
                value={date}
                onChange={(event) => setDate(event.target.value)}
              />
            </Field>
          </div>

          <Field label="Description">
            <input
              className="rawjan-input"
              placeholder={isTransfer ? "Move to savings" : isIncome ? "Monthly salary" : "Groceries at the market"}
              value={description}
              onChange={(event) => setDescription(event.target.value)}
            />
          </Field>

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Account">
              <select
                className="rawjan-input"
                value={accountId}
                onChange={(event) => setAccountId(event.target.value)}
              >
                {activeAccounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name} · {account.currency}
                  </option>
                ))}
              </select>
            </Field>
            {needsCounter ? (
              <Field label="Destination account">
                <select
                  className="rawjan-input"
                  value={counterAccountId}
                  onChange={(event) => setCounterAccountId(event.target.value)}
                >
                  <option value="">Select destination…</option>
                  {activeAccounts
                    .filter((account) => account.id !== accountId)
                    .map((account) => (
                      <option key={account.id} value={account.id}>
                        {account.name} · {account.currency}
                      </option>
                    ))}
                </select>
              </Field>
            ) : (
              <Field label="Category">
                <select
                  className="rawjan-input"
                  value={categoryId}
                  onChange={(event) => setCategoryId(event.target.value)}
                >
                  <option value="">Select category…</option>
                  {categoryOptions.map((category) => (
                    <option key={category.id} value={category.id}>
                      {category.parentId ? `— ${category.name}` : category.name}
                    </option>
                  ))}
                </select>
              </Field>
            )}
          </div>

          {isIncome ? (
            <Field
              label="Income source"
              hint={
                incomeSources.length === 0
                  ? "No sources yet. Add one on the Income page to compare expected and received amounts."
                  : "Optional. Links this entry to an income source for expected-versus-received reporting."
              }
            >
              <select
                className="rawjan-input"
                value={incomeSourceId}
                onChange={(event) => selectIncomeSource(event.target.value)}
              >
                <option value="">Unassigned income</option>
                {incomeSources.map((source) => (
                  <option key={source.id} value={source.id}>
                    {source.name}
                    {source.schedule ? ` · ${source.schedule}` : ""}
                  </option>
                ))}
              </select>
            </Field>
          ) : null}

          {crossCurrency ? (
            <div className="grid gap-3 sm:grid-cols-2">
              <Field
                label={`Amount received (${selectedCounter?.currency})`}
                hint="Required because source and destination use different currencies."
              >
                <input
                  className="rawjan-input font-mono"
                  inputMode="decimal"
                  placeholder="0.00"
                  value={counterAmount}
                  onChange={(event) => setCounterAmount(event.target.value)}
                />
              </Field>
              <div className="hidden sm:block" />
            </div>
          ) : null}

          {foreignAccount && !isTransfer ? (
            <Field
              label={`Exchange rate (${selectedAccount?.currency} → ${app.primaryCurrency})`}
              hint="Optional. Leave blank to use the most recent stored rate. A manual rate is saved as an immutable snapshot."
            >
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="e.g. 3.64"
                value={fxRate}
                onChange={(event) => setFxRate(event.target.value)}
              />
            </Field>
          ) : null}

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Payee" hint="Optional.">
              <input
                className="rawjan-input"
                placeholder="Merchant, employer or person"
                value={payee}
                onChange={(event) => setPayee(event.target.value)}
              />
            </Field>
            <Field label="Tags" hint="Comma separated. Created automatically.">
              <input
                className="rawjan-input"
                placeholder="essential, weekly"
                value={tags}
                onChange={(event) => setTags(event.target.value)}
              />
            </Field>
          </div>

          <Field label="Notes" hint="Optional.">
            <textarea
              className="rawjan-input min-h-[72px] resize-y"
              placeholder="Anything worth remembering about this entry."
              value={notes}
              onChange={(event) => setNotes(event.target.value)}
            />
          </Field>

          <Field label="Record as">
            <select
              className="rawjan-input"
              value={status}
              onChange={(event) => setStatus(event.target.value)}
            >
              {STATUS_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </Field>

          {selectedAccount && selectedCounter && isTransfer ? (
            <div className="flex items-center gap-2 rounded-xl border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[0.7rem] text-zinc-400">
              <span className="truncate">{selectedAccount.name}</span>
              <ArrowRight className="h-3 w-3 shrink-0 text-zinc-600" />
              <span className="truncate">{selectedCounter.name}</span>
            </div>
          ) : null}

          {formError ? (
            <Alert tone="error" title="Entry not saved">
              <span className="inline-flex items-start gap-1.5">
                <CircleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                <span>{formError}</span>
              </span>
            </Alert>
          ) : null}
        </div>
      )}
    </Modal>
  );
}
