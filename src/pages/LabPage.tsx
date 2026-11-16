import { useMemo, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import clsx from "clsx";
import { FlaskConical, Landmark, Percent, TrendingUp } from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type {
  CompoundInput,
  CompoundReport,
  InflationInput,
  InflationReport,
  LoanInput,
  LoanReport,
} from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { money } from "@/lib/display";
import { Alert, Button, EmptyState, Field, Panel, SectionLabel, Spinner } from "@/components/ui";

type Tab = "compound" | "loan" | "inflation";

const TABS: Array<{ id: Tab; label: string; icon: typeof TrendingUp }> = [
  { id: "compound", label: "Compound growth", icon: TrendingUp },
  { id: "loan", label: "Loan amortisation", icon: Landmark },
  { id: "inflation", label: "Inflation", icon: Percent },
];

export function LabPage() {
  const [tab, setTab] = useState<Tab>("compound");

  return (
    <div className="space-y-4 rawjan-enter">
      <div>
        <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
          Scenarios, kept separate
        </div>
        <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Financial lab</h2>
        <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
          Deterministic calculators that never touch your ledger. Results are projections from the numbers you
          enter, with simplified assumptions stated below each one. Nothing here is a forecast or a promise.
        </p>
      </div>

      <div className="flex flex-wrap gap-1.5">
        {TABS.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setTab(item.id)}
            className={clsx(
              "inline-flex items-center gap-2 rounded-lg border px-3 py-2 text-[0.72rem] transition",
              tab === item.id
                ? "border-lily-500/30 bg-lily-600/10 text-zinc-100"
                : "border-white/[0.07] text-zinc-500 hover:border-white/15 hover:text-zinc-300",
            )}
          >
            <item.icon className="h-3.5 w-3.5" />
            {item.label}
          </button>
        ))}
      </div>

      {tab === "compound" ? <CompoundPanel /> : null}
      {tab === "loan" ? <LoanPanel /> : null}
      {tab === "inflation" ? <InflationPanel /> : null}

      <div className="flex items-center gap-1.5 text-[0.6rem] text-zinc-700">
        <FlaskConical className="h-3 w-3" />
        All arithmetic runs in Rust using decimal values. The interface only renders the results.
      </div>
    </div>
  );
}

function Notes({ notes }: { notes: string[] }) {
  return (
    <ul className="mt-3 space-y-1.5 border-t border-white/[0.05] pt-3 text-[0.62rem] leading-relaxed text-zinc-600">
      {notes.map((note) => (
        <li key={note} className="flex gap-2">
          <span className="mt-1.5 h-1 w-1 shrink-0 rounded-full bg-zinc-700" />
          <span>{note}</span>
        </li>
      ))}
    </ul>
  );
}

function Stat({ label, value, tone }: { label: string; value: string; tone?: string }) {
  return (
    <div className="rounded-lg border border-white/[0.06] bg-black/20 px-3 py-2">
      <div className="text-[0.54rem] uppercase tracking-[0.14em] text-zinc-600">{label}</div>
      <div className={clsx("mt-0.5 truncate font-mono text-[0.68rem]", tone ?? "text-zinc-300")}>{value}</div>
    </div>
  );
}

function CompoundPanel() {
  const { app } = useBootstrap();
  const [form, setForm] = useState<CompoundInput>({
    initial: "",
    monthlyContribution: "",
    annualReturn: "6",
    years: 10,
  });
  const [report, setReport] = useState<CompoundReport | null>(null);

  const mutation = useMutation({
    mutationFn: (input: CompoundInput) => api.labCompound(input),
    onSuccess: (result) => setReport(result),
  });

  return (
    <div className="grid gap-3 lg:grid-cols-[0.85fr_1.15fr]">
      <Panel className="p-5">
        <SectionLabel>Inputs</SectionLabel>
        <div className="mt-3 space-y-3">
          <Field label="Starting balance" hint={`In ${app.primaryCurrency}`}>
            <input
              className="rawjan-input font-mono"
              inputMode="decimal"
              placeholder="0"
              value={form.initial}
              onChange={(event) => setForm((current) => ({ ...current, initial: event.target.value }))}
            />
          </Field>
          <Field label="Monthly contribution" hint="Added at the end of each month">
            <input
              className="rawjan-input font-mono"
              inputMode="decimal"
              placeholder="0"
              value={form.monthlyContribution}
              onChange={(event) =>
                setForm((current) => ({ ...current, monthlyContribution: event.target.value }))
              }
            />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label="Annual return %">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                value={form.annualReturn}
                onChange={(event) => setForm((current) => ({ ...current, annualReturn: event.target.value }))}
              />
            </Field>
            <Field label="Years" hint="1 – 60">
              <input
                className="rawjan-input font-mono"
                inputMode="numeric"
                value={String(form.years)}
                onChange={(event) =>
                  setForm((current) => ({ ...current, years: Number(event.target.value) || 0 }))
                }
              />
            </Field>
          </div>
        </div>
        {mutation.isError ? (
          <div className="mt-3">
            <Alert tone="error" title="Could not calculate">
              {errorMessage(mutation.error)}
            </Alert>
          </div>
        ) : null}
        <Button
          className="mt-4 w-full"
          variant="primary"
          size="sm"
          loading={mutation.isPending}
          onClick={() => mutation.mutate(form)}
        >
          Project growth
        </Button>
        <Notes
          notes={[
            "Returns are applied monthly at one twelfth of the annual rate.",
            "Contributions are level and made at month end. Taxes, fees and volatility are not modelled.",
          ]}
        />
      </Panel>

      <Panel className="p-5">
        {mutation.isPending ? (
          <div className="flex h-full items-center justify-center py-16 text-zinc-500">
            <Spinner className="h-5 w-5" />
          </div>
        ) : !report ? (
          <EmptyState
            className="border-0 py-12"
            icon={<TrendingUp className="h-5 w-5" />}
            title="No projection yet"
            description="Enter a starting balance and contribution, then project. The result is arithmetic, not a market forecast."
          />
        ) : (
          <div className="space-y-4">
            <div>
              <SectionLabel>After {report.points[report.points.length - 1]?.year ?? 0} years</SectionLabel>
              <div className="mt-1.5 font-display text-[1.9rem] leading-none text-zinc-50">
                {money(report.finalAmount, report.currency)}
              </div>
            </div>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
              <Stat label="Contributed" value={money(report.contributed, report.currency)} />
              <Stat
                label="Growth"
                value={money(report.growth, report.currency)}
                tone={report.growthMinor >= 0 ? "text-sage-400" : "text-lily-300"}
              />
              <Stat label="Currency" value={report.currency} />
            </div>
            <CompoundChart report={report} />
            <div className="max-h-[220px] overflow-y-auto rounded-lg border border-white/[0.05]">
              <table className="w-full border-collapse text-left text-[0.66rem]">
                <thead className="sticky top-0 bg-ink-900/95 text-[0.54rem] uppercase tracking-[0.12em] text-zinc-600">
                  <tr>
                    <th scope="col" className="px-3 py-2 font-medium">Year</th>
                    <th scope="col" className="px-3 py-2 text-right font-medium">Balance</th>
                    <th scope="col" className="px-3 py-2 text-right font-medium">Contributed</th>
                  </tr>
                </thead>
                <tbody>
                  {report.points.map((point) => (
                    <tr key={point.year} className="border-t border-white/[0.04]">
                      <td className="px-3 py-1.5 text-zinc-500">{point.year}</td>
                      <td className="px-3 py-1.5 text-right font-mono text-zinc-300">{point.balance}</td>
                      <td className="px-3 py-1.5 text-right font-mono text-zinc-500">{point.contributed}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <Notes notes={report.notes} />
          </div>
        )}
      </Panel>
    </div>
  );
}

function CompoundChart({ report }: { report: CompoundReport }) {
  const points = report.points;
  const width = 600;
  const height = 170;
  const padX = 10;
  const padY = 14;
  const max = useMemo(
    () => points.reduce((peak, point) => Math.max(peak, point.balanceMinor, point.contributedMinor), 0),
    [points],
  );
  if (points.length < 2 || max <= 0) return null;
  const stepX = (width - padX * 2) / (points.length - 1);
  const yFor = (value: number) => padY + (1 - value / max) * (height - padY * 2);
  const build = (key: "balanceMinor" | "contributedMinor") =>
    points
      .map((point, index) => `${(padX + index * stepX).toFixed(1)},${yFor(point[key]).toFixed(1)}`)
      .join(" ");

  return (
    <div>
      <svg viewBox={`0 0 ${width} ${height}`} className="h-[170px] w-full" role="img" aria-label="Balance against contributions">
        <polyline
          points={build("contributedMinor")}
          fill="none"
          stroke="rgb(122 122 140)"
          strokeWidth="1.4"
          strokeDasharray="4 4"
        />
        <polyline
          points={build("balanceMinor")}
          fill="none"
          stroke="rgb(124 143 106)"
          strokeWidth="1.9"
          strokeLinejoin="round"
          strokeLinecap="round"
        />
      </svg>
      <div className="flex items-center justify-between text-[0.58rem] text-zinc-600">
        <span className="inline-flex items-center gap-1.5">
          <span className="h-[1px] w-4 bg-sage-500" /> Balance
        </span>
        <span className="inline-flex items-center gap-1.5">
          <span className="h-[1px] w-4 border-t border-dashed border-zinc-500" /> Contributed
        </span>
        <span>Year {points[points.length - 1].year}</span>
      </div>
    </div>
  );
}

function LoanPanel() {
  const { app } = useBootstrap();
  const [form, setForm] = useState<LoanInput>({ principal: "", annualRate: "5", months: 60 });
  const [report, setReport] = useState<LoanReport | null>(null);

  const mutation = useMutation({
    mutationFn: (input: LoanInput) => api.labLoan(input),
    onSuccess: (result) => setReport(result),
  });

  return (
    <div className="grid gap-3 lg:grid-cols-[0.85fr_1.15fr]">
      <Panel className="p-5">
        <SectionLabel>Inputs</SectionLabel>
        <div className="mt-3 space-y-3">
          <Field label="Loan principal" hint={`In ${app.primaryCurrency}`}>
            <input
              className="rawjan-input font-mono"
              inputMode="decimal"
              placeholder="0"
              value={form.principal}
              onChange={(event) => setForm((current) => ({ ...current, principal: event.target.value }))}
            />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label="Annual rate %">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                value={form.annualRate}
                onChange={(event) => setForm((current) => ({ ...current, annualRate: event.target.value }))}
              />
            </Field>
            <Field label="Term (months)" hint="1 – 600">
              <input
                className="rawjan-input font-mono"
                inputMode="numeric"
                value={String(form.months)}
                onChange={(event) =>
                  setForm((current) => ({ ...current, months: Number(event.target.value) || 0 }))
                }
              />
            </Field>
          </div>
        </div>
        {mutation.isError ? (
          <div className="mt-3">
            <Alert tone="error" title="Could not calculate">
              {errorMessage(mutation.error)}
            </Alert>
          </div>
        ) : null}
        <Button
          className="mt-4 w-full"
          variant="primary"
          size="sm"
          loading={mutation.isPending}
          onClick={() => mutation.mutate(form)}
        >
          Build schedule
        </Button>
        <Notes
          notes={[
            "Fixed-rate amortisation with payments rounded to the currency's minor unit.",
            "The final payment clears the remaining balance. No fees, insurance or rate changes are modelled.",
          ]}
        />
      </Panel>

      <Panel className="p-5">
        {mutation.isPending ? (
          <div className="flex h-full items-center justify-center py-16 text-zinc-500">
            <Spinner className="h-5 w-5" />
          </div>
        ) : !report ? (
          <EmptyState
            className="border-0 py-12"
            icon={<Landmark className="h-5 w-5" />}
            title="No schedule yet"
            description="Enter a principal, rate and term to see the fixed payment and the month-by-month split."
          />
        ) : (
          <div className="space-y-4">
            <div>
              <SectionLabel>Monthly payment</SectionLabel>
              <div className="mt-1.5 font-display text-[1.9rem] leading-none text-zinc-50">
                {money(report.monthlyPayment, report.currency)}
              </div>
              <div className="mt-1 text-[0.62rem] text-zinc-600">{report.months} fixed payments</div>
            </div>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
              <Stat label="Total paid" value={money(report.totalPaid, report.currency)} />
              <Stat label="Total interest" value={money(report.totalInterest, report.currency)} tone="text-champagne-300" />
              <Stat label="Currency" value={report.currency} />
            </div>
            <div className="max-h-[300px] overflow-y-auto rounded-lg border border-white/[0.05]">
              <table className="w-full border-collapse text-left text-[0.66rem]">
                <thead className="sticky top-0 bg-ink-900/95 text-[0.54rem] uppercase tracking-[0.12em] text-zinc-600">
                  <tr>
                    <th scope="col" className="px-3 py-2 font-medium">Month</th>
                    <th scope="col" className="px-3 py-2 text-right font-medium">Payment</th>
                    <th scope="col" className="px-3 py-2 text-right font-medium">Interest</th>
                    <th scope="col" className="px-3 py-2 text-right font-medium">Principal</th>
                    <th scope="col" className="px-3 py-2 text-right font-medium">Balance</th>
                  </tr>
                </thead>
                <tbody>
                  {report.rows.map((row) => (
                    <tr key={row.month} className="border-t border-white/[0.04]">
                      <td className="px-3 py-1.5 text-zinc-500">{row.month}</td>
                      <td className="px-3 py-1.5 text-right font-mono text-zinc-300">{row.payment}</td>
                      <td className="px-3 py-1.5 text-right font-mono text-champagne-300/80">{row.interest}</td>
                      <td className="px-3 py-1.5 text-right font-mono text-zinc-400">{row.principal}</td>
                      <td className="px-3 py-1.5 text-right font-mono text-zinc-500">{row.balance}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <Notes notes={report.notes} />
          </div>
        )}
      </Panel>
    </div>
  );
}

function InflationPanel() {
  const { app } = useBootstrap();
  const [form, setForm] = useState<InflationInput>({ amount: "", annualInflation: "3", years: 10 });
  const [report, setReport] = useState<InflationReport | null>(null);
  const [submitted, setSubmitted] = useState<InflationInput | null>(null);

  const mutation = useMutation({
    mutationFn: (input: InflationInput) => api.labInflation(input),
    onSuccess: (result, input) => {
      setReport(result);
      setSubmitted(input);
    },
  });

  return (
    <div className="grid gap-3 lg:grid-cols-[0.85fr_1.15fr]">
      <Panel className="p-5">
        <SectionLabel>Inputs</SectionLabel>
        <div className="mt-3 space-y-3">
          <Field label="Amount today" hint={`In ${app.primaryCurrency}`}>
            <input
              className="rawjan-input font-mono"
              inputMode="decimal"
              placeholder="0"
              value={form.amount}
              onChange={(event) => setForm((current) => ({ ...current, amount: event.target.value }))}
            />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label="Annual inflation %">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                value={form.annualInflation}
                onChange={(event) =>
                  setForm((current) => ({ ...current, annualInflation: event.target.value }))
                }
              />
            </Field>
            <Field label="Years" hint="0 – 80">
              <input
                className="rawjan-input font-mono"
                inputMode="numeric"
                value={String(form.years)}
                onChange={(event) =>
                  setForm((current) => ({ ...current, years: Number(event.target.value) || 0 }))
                }
              />
            </Field>
          </div>
        </div>
        {mutation.isError ? (
          <div className="mt-3">
            <Alert tone="error" title="Could not calculate">
              {errorMessage(mutation.error)}
            </Alert>
          </div>
        ) : null}
        <Button
          className="mt-4 w-full"
          variant="primary"
          size="sm"
          loading={mutation.isPending}
          onClick={() => mutation.mutate(form)}
        >
          Adjust for inflation
        </Button>
        <Notes
          notes={[
            "Future cost compounds the entered rate yearly.",
            "Real value discounts a future amount back to today's money. Inflation is assumed constant.",
          ]}
        />
      </Panel>

      <Panel className="p-5">
        {mutation.isPending ? (
          <div className="flex h-full items-center justify-center py-16 text-zinc-500">
            <Spinner className="h-5 w-5" />
          </div>
        ) : !report ? (
          <EmptyState
            className="border-0 py-12"
            icon={<Percent className="h-5 w-5" />}
            title="No comparison yet"
            description="Enter an amount and a constant inflation rate to see what it costs later and what a future amount is worth today."
          />
        ) : (
          <div className="space-y-4">
            <div>
              <SectionLabel>Future cost</SectionLabel>
              <div className="mt-1.5 font-display text-[1.9rem] leading-none text-zinc-50">
                {money(report.futureCost, report.currency)}
              </div>
              <div className="mt-1 text-[0.62rem] text-zinc-600">
                after {submitted?.years ?? 0} year{(submitted?.years ?? 0) === 1 ? "" : "s"} at{" "}
                {submitted?.annualInflation ?? "—"}%
              </div>
            </div>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
              <Stat label="Increase" value={money(report.increase, report.currency)} tone="text-champagne-300" />
              <Stat label="Worth today" value={money(report.realValue, report.currency)} />
              <Stat label="Currency" value={report.currency} />
            </div>
            <div className="rounded-xl border border-white/[0.06] bg-white/[0.015] p-4 text-[0.7rem] leading-relaxed text-zinc-500">
              A future amount of {money(report.futureCost, report.currency)} has the purchasing power of{" "}
              {money(report.realValue, report.currency)} in today's money, assuming the same constant rate.
            </div>
            <Notes notes={report.notes} />
          </div>
        )}
      </Panel>
    </div>
  );
}
