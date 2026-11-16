import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import {
  Archive,
  Banknote,
  CreditCard,
  Landmark,
  PiggyBank,
  Plus,
  Search,
  TrendingUp,
  Wallet,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { Account, AccountInput } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { amountTone, formatDate, signedAmount, titleCase } from "@/lib/display";
import { useToast } from "@/components/Toast";
import { TransactionDetailModal } from "@/components/TransactionDetail";
import { Alert, Badge, Button, EmptyState, Field, Modal, Panel, Spinner } from "@/components/ui";

const ACCOUNT_TYPES = [
  { value: "bank", label: "Bank account" },
  { value: "cash", label: "Cash" },
  { value: "debit_card", label: "Debit card" },
  { value: "credit_card", label: "Credit card" },
  { value: "savings", label: "Savings" },
  { value: "wallet", label: "Wallet" },
  { value: "investment", label: "Investment" },
  { value: "loan", label: "Loan" },
  { value: "other", label: "Other" },
];

const TYPE_ICON: Record<string, typeof Wallet> = {
  bank: Landmark,
  cash: Banknote,
  debit_card: CreditCard,
  credit_card: CreditCard,
  savings: PiggyBank,
  wallet: Wallet,
  investment: TrendingUp,
  loan: Landmark,
  other: Wallet,
};

interface AccountFormState {
  name: string;
  accountType: string;
  institution: string;
  currencyCode: string;
  openingBalance: string;
  openingDate: string;
  creditLimit: string;
  statementDay: string;
  paymentDueDay: string;
  notes: string;
}

export function AccountsPage() {
  const { accounts, currencies, app } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();
  const [createOpen, setCreateOpen] = useState(false);
  const [registerAccount, setRegisterAccount] = useState<Account | null>(null);
  const [detailId, setDetailId] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const [form, setForm] = useState<AccountFormState>({
    name: "",
    accountType: "bank",
    institution: "",
    currencyCode: app.primaryCurrency,
    openingBalance: "",
    openingDate: app.today,
    creditLimit: "",
    statementDay: "",
    paymentDueDay: "",
    notes: "",
  });

  const allAccounts = useQuery({
    queryKey: ["accounts", { includeArchived: true }],
    queryFn: () => api.listAccounts(true),
  });

  const archiveMutation = useMutation({
    mutationFn: ({ id, archive }: { id: string; archive: boolean }) => api.archiveAccount(id, archive),
    onSuccess: async (_data, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["accounts"] }),
        queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
        queryClient.invalidateQueries({ queryKey: ["transactions"] }),
      ]);
      toast.success(variables.archive ? "Account archived" : "Account restored");
    },
    onError: (error) => toast.error("Account not changed", errorMessage(error)),
  });

  const createMutation = useMutation({
    mutationFn: (input: AccountInput) => api.createAccount(input),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["accounts"] }),
        queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
        queryClient.invalidateQueries({ queryKey: ["transactions"] }),
      ]);
      toast.success("Account created", "Balances are derived from the ledger, never stored twice.");
      setCreateOpen(false);
    },
    onError: (error) => setFormError(errorMessage(error)),
  });

  const source = showArchived ? (allAccounts.data ?? accounts) : accounts;
  const visible = useMemo(() => {
    const term = search.trim().toLowerCase();
    return source.filter((account) => {
      if (!term) return true;
      return (
        account.name.toLowerCase().includes(term) ||
        (account.institution ?? "").toLowerCase().includes(term) ||
        account.currency.toLowerCase().includes(term)
      );
    });
  }, [source, search]);

  const grouped = useMemo(() => {
    const assets = visible.filter((account) => !account.isLiability);
    const liabilities = visible.filter((account) => account.isLiability);
    const investments = assets.filter((account) => account.accountType === "investment");
    const liquid = assets.filter((account) => account.accountType !== "investment");
    return { liquid, investments, liabilities };
  }, [visible]);

  const openCreate = () => {
    setForm({
      name: "",
      accountType: "bank",
      institution: "",
      currencyCode: app.primaryCurrency,
      openingBalance: "",
      openingDate: app.today,
      creditLimit: "",
      statementDay: "",
      paymentDueDay: "",
      notes: "",
    });
    setFormError(null);
    setCreateOpen(true);
  };

  const submit = () => {
    setFormError(null);
    if (!form.name.trim()) {
      setFormError("Give the account a name.");
      return;
    }
    const input: AccountInput = {
      name: form.name.trim(),
      accountType: form.accountType,
      institution: form.institution.trim() || null,
      currencyCode: form.currencyCode,
      openingBalance: form.openingBalance.trim() || null,
      openingDate: form.openingDate || null,
      creditLimit: form.creditLimit.trim() || null,
      statementDay: form.statementDay ? Number(form.statementDay) : null,
      paymentDueDay: form.paymentDueDay ? Number(form.paymentDueDay) : null,
      notes: form.notes.trim() || null,
    };
    createMutation.mutate(input);
  };

  const includesArchived = showArchived;

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Balances derived from the ledger
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Accounts</h2>
          <p className="mt-1 max-w-2xl text-xs leading-relaxed text-zinc-500">
            Accounts hold original-currency balances. Net worth converts to {app.primaryCurrency} only with stored
            exchange rates; missing rates are disclosed, never guessed.
          </p>
        </div>
        <Button size="sm" variant="primary" onClick={openCreate}>
          <Plus className="h-3.5 w-3.5" /> New account
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-[220px] flex-1 sm:max-w-sm">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-zinc-600" />
          <input
            className="rawjan-input h-9 pl-8 text-xs"
            placeholder="Search accounts…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>
        <button
          type="button"
          onClick={() => setShowArchived((value) => !value)}
          className={clsx(
            "rounded-lg border px-3 py-1.5 text-[0.68rem] transition",
            includesArchived
              ? "border-lily-500/40 bg-lily-600/10 text-lily-200"
              : "border-white/[0.08] text-zinc-500 hover:text-zinc-200",
          )}
        >
          <Archive className="mr-1 inline h-3 w-3" />
          {includesArchived ? "Showing archived" : "Show archived"}
        </button>
        {allAccounts.isFetching ? <Spinner className="h-3.5 w-3.5 text-zinc-600" /> : null}
      </div>

      {allAccounts.isError ? (
        <Alert tone="error" title="Could not load accounts">
          {errorMessage(allAccounts.error)}
        </Alert>
      ) : null}

      {visible.length === 0 ? (
        <EmptyState
          icon={<Wallet className="h-5 w-5" />}
          title={search ? "No accounts match" : "No accounts yet"}
          description={
            search
              ? "Try a different name, institution or currency."
              : "Create a bank account, cash wallet or card. An optional opening balance is posted as a real ledger entry."
          }
          action={
            search ? null : (
              <Button size="sm" variant="primary" onClick={openCreate}>
                <Plus className="h-3.5 w-3.5" /> Create first account
              </Button>
            )
          }
        />
      ) : (
        <div className="space-y-5">
          <AccountGroup
            label="Cash & bank"
            accounts={grouped.liquid}
            primaryCurrency={app.primaryCurrency}
            onOpenRegister={setRegisterAccount}
            onArchive={(account, archive) => archiveMutation.mutate({ id: account.id, archive })}
            busy={archiveMutation.isPending}
          />
          <AccountGroup
            label="Investments"
            accounts={grouped.investments}
            primaryCurrency={app.primaryCurrency}
            onOpenRegister={setRegisterAccount}
            onArchive={(account, archive) => archiveMutation.mutate({ id: account.id, archive })}
            busy={archiveMutation.isPending}
          />
          <AccountGroup
            label="Liabilities"
            accounts={grouped.liabilities}
            primaryCurrency={app.primaryCurrency}
            onOpenRegister={setRegisterAccount}
            onArchive={(account, archive) => archiveMutation.mutate({ id: account.id, archive })}
            busy={archiveMutation.isPending}
          />
        </div>
      )}

      <Modal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        title="New account"
        description="Opening balances are posted as an explicit opening transaction. Nothing is fabricated or pre-filled."
        width="lg"
        footer={
          <>
            <Button variant="ghost" onClick={() => setCreateOpen(false)} disabled={createMutation.isPending}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submit} loading={createMutation.isPending}>
              Create account
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Account name">
              <input
                className="rawjan-input"
                placeholder="e.g. QIB current account"
                value={form.name}
                onChange={(event) => setForm({ ...form, name: event.target.value })}
                autoFocus
              />
            </Field>
            <Field label="Type">
              <select
                className="rawjan-input"
                value={form.accountType}
                onChange={(event) => setForm({ ...form, accountType: event.target.value })}
              >
                {ACCOUNT_TYPES.map((type) => (
                  <option key={type.value} value={type.value}>
                    {type.label}
                  </option>
                ))}
              </select>
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Institution" hint="Optional. No connection is created.">
              <input
                className="rawjan-input"
                placeholder="QIB, cash, Trading 212…"
                value={form.institution}
                onChange={(event) => setForm({ ...form, institution: event.target.value })}
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
            <Field
              label="Opening balance"
              hint="Optional. Positive for assets, negative for amounts owed. Posted as an opening entry."
            >
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={form.openingBalance}
                onChange={(event) => setForm({ ...form, openingBalance: event.target.value })}
              />
            </Field>
            <Field label="Opening date">
              <input
                type="date"
                className="rawjan-input"
                value={form.openingDate}
                onChange={(event) => setForm({ ...form, openingDate: event.target.value })}
              />
            </Field>
          </div>

          {form.accountType === "credit_card" ? (
            <div className="grid gap-3 sm:grid-cols-3">
              <Field label="Credit limit">
                <input
                  className="rawjan-input font-mono"
                  inputMode="decimal"
                  placeholder="0.00"
                  value={form.creditLimit}
                  onChange={(event) => setForm({ ...form, creditLimit: event.target.value })}
                />
              </Field>
              <Field label="Statement day">
                <input
                  className="rawjan-input"
                  type="number"
                  min={1}
                  max={31}
                  placeholder="1–31"
                  value={form.statementDay}
                  onChange={(event) => setForm({ ...form, statementDay: event.target.value })}
                />
              </Field>
              <Field label="Payment due day">
                <input
                  className="rawjan-input"
                  type="number"
                  min={1}
                  max={31}
                  placeholder="1–31"
                  value={form.paymentDueDay}
                  onChange={(event) => setForm({ ...form, paymentDueDay: event.target.value })}
                />
              </Field>
            </div>
          ) : null}

          <Field label="Notes" hint="Optional.">
            <textarea
              className="rawjan-input min-h-[68px] resize-y"
              value={form.notes}
              onChange={(event) => setForm({ ...form, notes: event.target.value })}
            />
          </Field>

          <Alert tone="info">
            {form.accountType === "credit_card" || form.accountType === "loan"
              ? "This account is treated as a liability. Available credit is calculated from the ledger, not stored as a separate number."
              : "This account is treated as an asset and included in liquid cash unless its type is investment."}
          </Alert>

          {formError ? (
            <Alert tone="error" title="Account not created">
              {formError}
            </Alert>
          ) : null}
        </div>
      </Modal>

      <AccountRegisterModal
        account={registerAccount}
        onClose={() => setRegisterAccount(null)}
        onOpenDetail={(id) => {
          setRegisterAccount(null);
          setDetailId(id);
        }}
      />

      <TransactionDetailModal id={detailId} onClose={() => setDetailId(null)} />
    </div>
  );
}

function AccountGroup({
  label,
  accounts,
  primaryCurrency,
  onOpenRegister,
  onArchive,
  busy,
}: {
  label: string;
  accounts: Account[];
  primaryCurrency: string;
  onOpenRegister: (account: Account) => void;
  onArchive: (account: Account, archive: boolean) => void;
  busy: boolean;
}) {
  if (accounts.length === 0) return null;
  return (
    <section>
      <div className="mb-2 flex items-center gap-2">
        <span className="text-[0.58rem] font-semibold uppercase tracking-[0.18em] text-zinc-600">{label}</span>
        <span className="text-[0.58rem] text-zinc-700">{accounts.length}</span>
      </div>
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
        {accounts.map((account) => (
          <AccountCard
            key={account.id}
            account={account}
            primaryCurrency={primaryCurrency}
            onOpenRegister={onOpenRegister}
            onArchive={onArchive}
            busy={busy}
          />
        ))}
      </div>
    </section>
  );
}

function AccountCard({
  account,
  primaryCurrency,
  onOpenRegister,
  onArchive,
  busy,
}: {
  account: Account;
  primaryCurrency: string;
  onOpenRegister: (account: Account) => void;
  onArchive: (account: Account, archive: boolean) => void;
  busy: boolean;
}) {
  const Icon = TYPE_ICON[account.accountType] ?? Wallet;
  const limit = account.creditLimitMinor;
  const used = limit && limit > 0 ? Math.min(100, Math.max(0, (Math.abs(account.balanceMinor) / limit) * 100)) : null;

  return (
    <Panel
      className={clsx(
        "flex h-full flex-col p-4 transition-colors hover:border-white/15",
        !account.isActive && "opacity-60",
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2.5">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-white/[0.08] bg-white/[0.025] text-lily-300">
            <Icon className="h-4 w-4" strokeWidth={1.8} />
          </div>
          <div className="min-w-0">
            <div className="truncate text-[0.84rem] text-zinc-100">{account.name}</div>
            <div className="truncate text-[0.62rem] text-zinc-600">
              {titleCase(account.accountType)}
              {account.institution ? ` · ${account.institution}` : ""}
            </div>
          </div>
        </div>
        <Badge tone={account.isActive ? "muted" : "default"}>{account.currency}</Badge>
      </div>

      <div className="mt-4">
        <div
          className={clsx(
            "font-display text-xl leading-tight",
            account.isLiability && account.balanceMinor !== 0 ? "text-lily-200" : "text-zinc-100",
          )}
        >
          {account.balance}{" "}
          <span className="text-[0.6rem] uppercase tracking-[0.14em] text-zinc-500">{account.currency}</span>
        </div>
        {account.currency !== primaryCurrency ? (
          <div className="mt-0.5 text-[0.62rem] text-zinc-600">
            {account.balanceBase !== null
              ? `≈ ${account.balanceBase} ${primaryCurrency}`
              : "No stored exchange rate — excluded from net worth"}
          </div>
        ) : null}
      </div>

      {used !== null ? (
        <div className="mt-3">
          <div className="flex items-center justify-between text-[0.6rem] text-zinc-600">
            <span>Credit used</span>
            <span>{Math.round(used)}%</span>
          </div>
          <div className="mt-1 h-1 overflow-hidden rounded-full bg-white/[0.05]">
            <div className="h-full rounded-full bg-lily-500/80" style={{ width: `${used}%` }} />
          </div>
          {account.available ? (
            <div className="mt-1 text-[0.6rem] text-zinc-600">Available {account.available} {account.currency}</div>
          ) : null}
        </div>
      ) : null}

      <div className="mt-auto flex items-center gap-1.5 pt-4">
        <Button size="sm" variant="secondary" onClick={() => onOpenRegister(account)}>
          Register
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={busy}
          onClick={() => {
            const action = account.isActive ? "Archive" : "Restore";
            if (window.confirm(`${action} “${account.name}”? Archived accounts keep their history and stay out of active totals.`)) {
              onArchive(account, account.isActive);
            }
          }}
        >
          <Archive className="h-3 w-3" />
          {account.isActive ? "Archive" : "Restore"}
        </Button>
      </div>
    </Panel>
  );
}

function AccountRegisterModal({
  account,
  onClose,
  onOpenDetail,
}: {
  account: Account | null;
  onClose: () => void;
  onOpenDetail: (id: string) => void;
}) {
  const register = useQuery({
    queryKey: ["account", account?.id, "register"],
    queryFn: () => api.accountRegister(account!.id, 100, 0),
    enabled: Boolean(account),
  });

  return (
    <Modal
      open={Boolean(account)}
      onClose={onClose}
      title={account ? `${account.name} register` : "Account register"}
      description={
        account
          ? `${account.balance} ${account.currency} · posted and reconciled entries`
          : undefined
      }
      width="lg"
      footer={
        <Button variant="ghost" onClick={onClose}>
          Close
        </Button>
      }
    >
      {register.isLoading ? (
        <div className="flex items-center justify-center py-12 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : register.isError ? (
        <Alert tone="error" title="Could not load register">
          {errorMessage(register.error)}
        </Alert>
      ) : (register.data?.rows.length ?? 0) === 0 ? (
        <EmptyState
          icon={<Wallet className="h-5 w-5" />}
          title="No posted entries"
          description="This account has no posted or reconciled transactions yet. Drafts and pending entries are visible in the full ledger."
        />
      ) : (
        <div className="overflow-hidden rounded-xl border border-white/[0.06]">
          {register.data?.rows.map((row, index) => (
            <button
              key={row.id}
              type="button"
              onClick={() => onOpenDetail(row.id)}
              className={clsx(
                "flex w-full items-center gap-3 px-3.5 py-2.5 text-left transition hover:bg-white/[0.03]",
                index > 0 && "border-t border-white/[0.05]",
              )}
            >
              <div className="w-[86px] shrink-0 text-[0.64rem] text-zinc-600">{formatDate(row.localDate)}</div>
              <div className="min-w-0 flex-1">
                <div className="truncate text-[0.74rem] text-zinc-300">{row.description}</div>
                <div className="truncate text-[0.6rem] text-zinc-600">
                  {row.categoryName ?? (row.counterAccountName ? `Transfer · ${row.counterAccountName}` : "—")}
                </div>
              </div>
              <div className={clsx("shrink-0 font-mono text-[0.72rem]", amountTone(row.kind))}>
                {signedAmount(row.kind, row.amount)} {row.accountCurrency}
              </div>
            </button>
          ))}
        </div>
      )}
    </Modal>
  );
}
