import { useState } from "react";
import type { ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import {
  Activity,
  CircleAlert,
  Database,
  Globe2,
  KeyRound,
  Plus,
  RefreshCw,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { AuditEvent, ExchangeRate, IntegrationStatus } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { formatDate, titleCase } from "@/lib/display";
import { useToast } from "@/components/Toast";
import { Alert, Badge, Button, Field, Panel, SectionLabel, Spinner } from "@/components/ui";

type Tab = "vault" | "preferences" | "currencies" | "integrations" | "activity";

const TABS: Array<{ id: Tab; label: string; icon: typeof ShieldCheck }> = [
  { id: "vault", label: "Vault & data", icon: Database },
  { id: "preferences", label: "Preferences", icon: SlidersHorizontal },
  { id: "currencies", label: "Currencies & rates", icon: Globe2 },
  { id: "integrations", label: "Integrations", icon: Settings2 },
  { id: "activity", label: "Audit trail", icon: Activity },
];

export function SettingsPage() {
  const [tab, setTab] = useState<Tab>("vault");

  return (
    <div className="space-y-4 rawjan-enter">
      <div>
        <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
          Local-first configuration
        </div>
        <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Settings</h2>
        <p className="mt-1 max-w-2xl text-xs leading-relaxed text-zinc-500">
          Rawjan stores everything on this device. Changes here are logged to the local audit trail. Optional
          integrations are shown with their real availability, never as working connections.
        </p>
      </div>

      <div className="flex flex-wrap gap-1.5">
        {TABS.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setTab(item.id)}
            className={clsx(
              "inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[0.7rem] transition",
              tab === item.id
                ? "border-lily-500/40 bg-lily-600/10 text-lily-100"
                : "border-white/[0.07] bg-white/[0.015] text-zinc-500 hover:border-white/15 hover:text-zinc-200",
            )}
          >
            <item.icon className="h-3.5 w-3.5" strokeWidth={1.8} />
            {item.label}
          </button>
        ))}
      </div>

      {tab === "vault" ? <VaultTab /> : null}
      {tab === "preferences" ? <PreferencesTab /> : null}
      {tab === "currencies" ? <CurrenciesTab /> : null}
      {tab === "integrations" ? <IntegrationsTab /> : null}
      {tab === "activity" ? <ActivityTab /> : null}
    </div>
  );
}

function VaultTab() {
  const { app, counts } = useBootstrap();
  const health = useQuery({ queryKey: ["health"], queryFn: () => api.health() });

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Panel className="p-5">
        <SectionLabel>Vault health</SectionLabel>
        {health.isLoading ? (
          <div className="flex items-center gap-2 py-6 text-xs text-zinc-500">
            <Spinner className="h-4 w-4" /> Running integrity check…
          </div>
        ) : health.isError ? (
          <Alert tone="error" title="Health check failed">
            {errorMessage(health.error)}
          </Alert>
        ) : (
          <div className="mt-3 space-y-2">
            <InfoRow
              icon={<ShieldCheck className="h-3.5 w-3.5 text-sage-400" />}
              label="Encryption at rest"
              value={health.data?.encrypted ? "SQLCipher (AES-256)" : "Disabled"}
              tone={health.data?.encrypted ? "green" : "red"}
            />
            <InfoRow
              icon={<Database className="h-3.5 w-3.5 text-champagne-400" />}
              label="SQLite integrity"
              value={health.data?.integrity ?? "—"}
              tone={health.data?.status === "ok" ? "green" : "amber"}
            />
            <InfoRow
              icon={<KeyRound className="h-3.5 w-3.5 text-zinc-500" />}
              label="Key storage"
              value="Windows Credential Manager"
              tone="default"
            />
            <InfoRow
              icon={<Database className="h-3.5 w-3.5 text-zinc-500" />}
              label="Schema version"
              value={String(app.schemaVersion)}
              tone="default"
            />
          </div>
        )}
        <div className="mt-4 rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-3 text-[0.66rem] leading-relaxed text-zinc-500">
          The database key is generated locally and held in the OS credential store. In portable mode, key
          handling is documented in the repository; the key is never written into the database file.
        </div>
      </Panel>

      <Panel className="p-5">
        <SectionLabel>Storage</SectionLabel>
        <div className="mt-3 space-y-2">
          <div className="rounded-xl border border-white/[0.06] bg-black/25 px-3.5 py-2.5">
            <div className="text-[0.58rem] uppercase tracking-[0.16em] text-zinc-600">Data directory</div>
            <div className="mt-1 break-all font-mono text-[0.68rem] text-zinc-400">
              {health.data?.dataDir ?? app.dataDir}
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <CountCell label="Transactions" value={counts.transactions} />
            <CountCell label="Accounts" value={counts.accounts} />
            <CountCell label="Budgets" value={counts.budgets} />
            <CountCell label="Goals" value={counts.goals} />
            <CountCell label="Investments" value={counts.instruments} />
            <CountCell label="Liabilities" value={counts.liabilities} />
          </div>
        </div>
        <div className="mt-4">
          <Alert tone="success" title="Backups and exports are available">
            Open <span className="font-medium text-zinc-100">Backups &amp; exports</span> in the
            sidebar to create a verified SQLCipher backup or an encrypted JSON export. Restoring is
            a documented manual step: close Rawjan, replace <span className="font-mono">rawjan.db</span>{" "}
            with the backup, and start it again. The key never leaves Windows Credential Manager.
          </Alert>
        </div>
      </Panel>

      <Panel className="p-5 lg:col-span-2">
        <SectionLabel>Migrations</SectionLabel>
        <div className="mt-2 text-[0.7rem] leading-relaxed text-zinc-500">
          Rawjan applies forward-only schema migrations on launch. A database created by a newer app version is
          refused rather than downgraded. This build supports schema {app.schemaVersion}. Financial records are
          never deleted by migrations; posted entries are reversed instead.
        </div>
      </Panel>
    </div>
  );
}

function PreferencesTab() {
  const { settings, currencies, app } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();
  const [error, setError] = useState<string | null>(null);

  const primary = String(settings.primary_currency ?? app.primaryCurrency);
  const cycleDay = Number(settings.budget_cycle_start_day ?? 28);
  const payDay = Number(settings.salary_pay_day ?? 28);
  const retention = Number(settings.backup_retention_days ?? 30);
  const locale = String(settings.locale ?? "en");
  const backupEnabled = Boolean(settings.backup_enabled ?? true);
  const emergencyMonths = Number(settings.emergency_fund_months ?? 6);

  const mutation = useMutation({
    mutationFn: ({ key, value }: { key: string; value: unknown }) => api.updateSetting(key, value),
    onSuccess: async () => {
      setError(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
        queryClient.invalidateQueries({ queryKey: ["audit"] }),
        queryClient.invalidateQueries({ queryKey: ["settings"] }),
      ]);
      toast.success("Preference saved");
    },
    onError: (mutationError) => {
      setError(errorMessage(mutationError));
      toast.error("Preference not saved", errorMessage(mutationError));
    },
  });

  const save = (key: string, value: unknown) => mutation.mutate({ key, value });

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Panel className="p-5">
        <SectionLabel>Regional</SectionLabel>
        <div className="mt-3 space-y-3">
          <Field label="Primary currency" hint="Reporting currency for net worth and base amounts.">
            <select
              className="rawjan-input"
              value={primary}
              onChange={(event) => save("primary_currency", event.target.value)}
              disabled={mutation.isPending}
            >
              {currencies.map((currency) => (
                <option key={currency.code} value={currency.code}>
                  {currency.code} — {currency.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Locale" hint="Display formatting hint. Amounts are always stored as minor units.">
            <select
              className="rawjan-input"
              value={locale}
              onChange={(event) => save("locale", event.target.value)}
              disabled={mutation.isPending}
            >
              <option value="en">English</option>
              <option value="ar">العربية (layout only)</option>
            </select>
          </Field>
          <div className="rounded-xl border border-amber-500/25 bg-amber-500/[0.06] px-3.5 py-2.5 text-[0.66rem] leading-relaxed text-amber-200/90">
            Changing the primary currency does not rewrite stored base amounts on existing transactions. New
            entries use the new reporting currency; historical snapshots stay as posted.
          </div>
        </div>
      </Panel>

      <Panel className="p-5">
        <SectionLabel>Budget cycle</SectionLabel>
        <div className="mt-3 space-y-3">
          <Field label="Cycle start day" hint="Defaults to 28 for a 28→27 salary cycle.">
            <input
              type="number"
              min={1}
              max={31}
              className="rawjan-input"
              defaultValue={cycleDay}
              disabled={mutation.isPending}
              onBlur={(event) => {
                const value = Number(event.target.value);
                if (Number.isFinite(value) && value !== cycleDay) save("budget_cycle_start_day", value);
              }}
            />
          </Field>
          <Field label="Salary pay day" hint="Used by budgets, calendar and cash-flow forecasts.">
            <input
              type="number"
              min={1}
              max={31}
              className="rawjan-input"
              defaultValue={payDay}
              disabled={mutation.isPending}
              onBlur={(event) => {
                const value = Number(event.target.value);
                if (Number.isFinite(value) && value !== payDay) save("salary_pay_day", value);
              }}
            />
          </Field>
          <Field label="Emergency fund target (months)">
            <input
              type="number"
              min={1}
              max={36}
              className="rawjan-input"
              defaultValue={emergencyMonths}
              disabled={mutation.isPending}
              onBlur={(event) => {
                const value = Number(event.target.value);
                if (Number.isFinite(value) && value !== emergencyMonths) save("emergency_fund_months", value);
              }}
            />
          </Field>
        </div>
      </Panel>

      <Panel className="p-5">
        <SectionLabel>Backups</SectionLabel>
        <div className="mt-3 space-y-3">
          <label className="flex items-center justify-between gap-3 rounded-xl border border-white/[0.06] bg-white/[0.015] px-3.5 py-2.5">
            <span>
              <span className="block text-[0.72rem] text-zinc-300">Scheduled backups</span>
              <span className="block text-[0.62rem] text-zinc-600">
                Preference recorded. Automatic scheduling is not built; create backups from the
                Backups &amp; exports page.
              </span>
            </span>
            <input
              type="checkbox"
              checked={backupEnabled}
              disabled={mutation.isPending}
              onChange={(event) => save("backup_enabled", event.target.checked)}
              className="h-4 w-4 accent-[#b0243a]"
            />
          </label>
          <Field label="Retention (days)">
            <input
              type="number"
              min={1}
              max={3650}
              className="rawjan-input"
              defaultValue={retention}
              disabled={mutation.isPending}
              onBlur={(event) => {
                const value = Number(event.target.value);
                if (Number.isFinite(value) && value !== retention) save("backup_retention_days", value);
              }}
            />
          </Field>
          <div className="rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-2.5 text-[0.66rem] leading-relaxed text-zinc-500">
            SQLCipher backups are taken through the app so the key and integrity checks are handled
            together. Each backup is verified immediately. A plain file copy is only safe while
            Rawjan is fully closed.
          </div>
        </div>
      </Panel>

      <Panel className="p-5">
        <SectionLabel>Safety</SectionLabel>
        <div className="mt-3 space-y-2">
          <InfoRow
            icon={<CircleAlert className="h-3.5 w-3.5 text-amber-400" />}
            label="App lock"
            value="Not implemented"
            tone="amber"
          />
          <InfoRow
            icon={<ShieldCheck className="h-3.5 w-3.5 text-sage-400" />}
            label="Network access"
            value="None in this build"
            tone="green"
          />
          <InfoRow
            icon={<RefreshCw className="h-3.5 w-3.5 text-zinc-500" />}
            label="Auto updates"
            value="Not configured"
            tone="default"
          />
        </div>
        {error ? (
          <div className="mt-3">
            <Alert tone="error" title="Preference not saved">
              {error}
            </Alert>
          </div>
        ) : null}
      </Panel>
    </div>
  );
}

function CurrenciesTab() {
  const { app, currencies } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();
  const [base, setBase] = useState("");
  const [rate, setRate] = useState("");
  const [asOfDate, setAsOfDate] = useState(app.today);
  const [error, setError] = useState<string | null>(null);

  const foreign = currencies.filter((currency) => currency.code !== app.primaryCurrency);
  const selectedBase = base || foreign[0]?.code || "";
  const quote = app.primaryCurrency;

  const rates = useQuery({
    queryKey: ["exchange-rates", quote],
    queryFn: () => api.listExchangeRates(undefined),
  });

  const addRate = useMutation({
    mutationFn: () => api.addExchangeRate(selectedBase, quote, rate.trim(), asOfDate),
    onSuccess: async () => {
      setRate("");
      setError(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["exchange-rates"] }),
        queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
        queryClient.invalidateQueries({ queryKey: ["audit"] }),
      ]);
      toast.success("Exchange rate saved", "The snapshot is immutable and used for new conversions.");
    },
    onError: (mutationError) => {
      setError(errorMessage(mutationError));
      toast.error("Rate not saved", errorMessage(mutationError));
    },
  });

  return (
    <div className="grid gap-4 lg:grid-cols-[1fr_1.3fr]">
      <Panel className="p-5">
        <SectionLabel>Add a manual rate</SectionLabel>
        <p className="mt-1 text-[0.66rem] leading-relaxed text-zinc-600">
          One unit of the foreign currency equals the rate in {quote}. Rates are timestamped snapshots; historical
          transactions keep the rate they were posted with.
        </p>
        <div className="mt-3 space-y-3">
          <Field label="Foreign currency">
            <select
              className="rawjan-input"
              value={selectedBase}
              onChange={(event) => setBase(event.target.value)}
            >
              {foreign.length === 0 ? <option value="">No foreign currencies active</option> : null}
              {foreign.map((currency) => (
                <option key={currency.code} value={currency.code}>
                  {currency.code} — {currency.name}
                </option>
              ))}
            </select>
          </Field>
          <Field label={`Rate (1 foreign = ? ${quote})`}>
            <input
              className="rawjan-input font-mono"
              inputMode="decimal"
              placeholder="e.g. 3.64"
              value={rate}
              onChange={(event) => setRate(event.target.value)}
            />
          </Field>
          <Field label="As of date">
            <input
              type="date"
              className="rawjan-input"
              value={asOfDate}
              onChange={(event) => setAsOfDate(event.target.value)}
            />
          </Field>
          {error ? (
            <Alert tone="error" title="Rate not saved">
              {error}
            </Alert>
          ) : null}
          <Button
            variant="primary"
            onClick={() => addRate.mutate()}
            loading={addRate.isPending}
            disabled={!selectedBase || !rate.trim()}
          >
            <Plus className="h-3.5 w-3.5" /> Save rate snapshot
          </Button>
        </div>
      </Panel>

      <Panel className="p-5">
        <div className="flex items-center justify-between">
          <SectionLabel>Stored rate snapshots</SectionLabel>
          <Button size="sm" variant="ghost" onClick={() => void rates.refetch()} disabled={rates.isFetching}>
            <RefreshCw className={clsx("h-3.5 w-3.5", rates.isFetching && "animate-spin")} />
          </Button>
        </div>
        {rates.isLoading ? (
          <div className="flex items-center gap-2 py-6 text-xs text-zinc-500">
            <Spinner className="h-4 w-4" /> Loading rates…
          </div>
        ) : rates.isError ? (
          <Alert tone="error" title="Could not load rates">
            {errorMessage(rates.error)}
          </Alert>
        ) : (rates.data?.length ?? 0) === 0 ? (
          <div className="mt-3 rounded-xl border border-dashed border-white/10 px-4 py-8 text-center text-xs text-zinc-500">
            No exchange-rate snapshots yet. Accounts in foreign currencies stay excluded from net worth until a
            rate is stored.
          </div>
        ) : (
          <div className="mt-3 overflow-hidden rounded-xl border border-white/[0.06]">
            <table className="w-full text-left text-[0.7rem]">
              <thead>
                <tr className="border-b border-white/[0.055] text-[0.56rem] uppercase tracking-[0.15em] text-zinc-600">
                  <th className="px-3 py-2 font-semibold">Pair</th>
                  <th className="px-2 py-2 text-right font-semibold">Rate</th>
                  <th className="px-2 py-2 font-semibold">As of</th>
                  <th className="px-3 py-2 font-semibold">Source</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/[0.04]">
                {rates.data?.map((snapshot: ExchangeRate) => (
                  <tr key={snapshot.id}>
                    <td className="px-3 py-2 font-mono text-zinc-300">
                      {snapshot.base}/{snapshot.quote}
                    </td>
                    <td className="px-2 py-2 text-right font-mono text-zinc-300">{snapshot.rate}</td>
                    <td className="px-2 py-2 text-zinc-500">{formatDate(snapshot.asOfDate)}</td>
                    <td className="px-3 py-2 text-[0.62rem] uppercase tracking-wider text-zinc-600">
                      {snapshot.source}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Panel>
    </div>
  );
}

function IntegrationsTab() {
  const integrations = useQuery({ queryKey: ["integrations"], queryFn: () => api.listIntegrationStatus() });

  return (
    <div className="space-y-3">
      <Alert tone="info" title="No third-party connection is active">
        Rawjan keeps your data on this device and never sends it to an external service. The items below
        describe the real capability limits of this build.
      </Alert>
      {integrations.isLoading ? (
        <div className="flex items-center gap-2 py-8 text-xs text-zinc-500">
          <Spinner className="h-4 w-4" /> Loading provider status…
        </div>
      ) : integrations.isError ? (
        <Alert tone="error" title="Could not load integrations">
          {errorMessage(integrations.error)}
        </Alert>
      ) : (
        <div className="grid gap-3 lg:grid-cols-2">
          {integrations.data?.map((provider: IntegrationStatus) => (
            <Panel key={provider.id} className="p-5">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="text-[0.84rem] text-zinc-100">{provider.name}</div>
                  <div className="mt-0.5 text-[0.6rem] uppercase tracking-[0.16em] text-zinc-600">
                    {titleCase(provider.category)}
                  </div>
                </div>
                <Badge tone={provider.available ? "green" : "amber"}>
                  {provider.available ? "Available" : "Unavailable"}
                </Badge>
              </div>
              <p className="mt-3 text-[0.7rem] leading-relaxed text-zinc-500">{provider.reason}</p>
              {provider.documentation ? (
                <a
                  href={provider.documentation}
                  target="_blank"
                  rel="noreferrer"
                  className="mt-3 inline-block text-[0.66rem] text-champagne-300 underline underline-offset-4 hover:text-champagne-200"
                >
                  Provider information
                </a>
              ) : null}
            </Panel>
          ))}
        </div>
      )}
    </div>
  );
}

function ActivityTab() {
  const audit = useQuery({
    queryKey: ["audit", 100],
    queryFn: () => api.listAuditEvents(100),
  });

  return (
    <Panel className="overflow-hidden">
      <div className="flex items-center justify-between gap-3 border-b border-white/[0.055] px-5 py-3.5">
        <div>
          <SectionLabel>Audit trail</SectionLabel>
          <div className="mt-0.5 text-[0.66rem] text-zinc-600">
            Append-only local record of mutations. Financial entries use reversal, not deletion.
          </div>
        </div>
        <Button size="sm" variant="ghost" onClick={() => void audit.refetch()} disabled={audit.isFetching}>
          <RefreshCw className={clsx("h-3.5 w-3.5", audit.isFetching && "animate-spin")} />
        </Button>
      </div>
      {audit.isLoading ? (
        <div className="flex items-center gap-2 px-5 py-8 text-xs text-zinc-500">
          <Spinner className="h-4 w-4" /> Loading audit events…
        </div>
      ) : audit.isError ? (
        <div className="p-5">
          <Alert tone="error" title="Could not load audit trail">
            {errorMessage(audit.error)}
          </Alert>
        </div>
      ) : (audit.data?.length ?? 0) === 0 ? (
        <div className="px-5 py-10 text-center text-xs text-zinc-500">
          No audit events yet. Creating accounts, settings or transactions will appear here.
        </div>
      ) : (
        <ul className="max-h-[560px] divide-y divide-white/[0.04] overflow-y-auto">
          {audit.data?.map((event: AuditEvent) => (
            <li key={event.id} className="flex items-start gap-3 px-5 py-2.5">
              <span className="mt-0.5 rounded-md border border-white/[0.07] bg-white/[0.02] px-1.5 py-0.5 font-mono text-[0.55rem] uppercase tracking-wider text-zinc-600">
                {event.entityType}
              </span>
              <div className="min-w-0 flex-1">
                <div className="text-[0.72rem] text-zinc-300">
                  {titleCase(event.action)}{" "}
                  <span className="text-zinc-600">{event.entityId.slice(0, 8)}</span>
                </div>
                <div className="truncate text-[0.6rem] text-zinc-600">
                  {event.createdAt.replace("T", " ").replace("Z", " UTC")}
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}

function InfoRow({
  icon,
  label,
  value,
  tone,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  tone: "default" | "green" | "amber" | "red";
}) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border border-white/[0.05] bg-white/[0.015] px-3 py-2">
      <span className="flex items-center gap-2 text-[0.7rem] text-zinc-400">
        {icon}
        {label}
      </span>
      <Badge tone={tone === "default" ? "muted" : tone}>{value}</Badge>
    </div>
  );
}

function CountCell({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-lg border border-white/[0.05] bg-white/[0.015] px-3 py-2">
      <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">{label}</div>
      <div className="mt-0.5 font-display text-base text-zinc-200">{value}</div>
    </div>
  );
}
