import { useMemo, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import clsx from "clsx";
import { Calculator, Sparkles } from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { FiInput, FiReport } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { formatDate, money } from "@/lib/display";
import { Alert, Badge, Button, EmptyState, Field, Panel, SectionLabel, Spinner } from "@/components/ui";

function numberSetting(value: unknown, fallback: string): string {
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  if (typeof value === "string" && value.trim() !== "" && Number.isFinite(Number(value))) {
    return String(Number(value));
  }
  return fallback;
}

function ProjectionChart({ report }: { report: FiReport }) {
  const points = report.projection;
  const width = 640;
  const height = 190;
  const padX = 12;
  const padY = 16;
  const max = useMemo(
    () => points.reduce((peak, point) => Math.max(peak, point.balanceMinor, point.targetMinor), 0),
    [points],
  );
  if (points.length < 2 || max <= 0) return null;

  const stepX = (width - padX * 2) / (points.length - 1);
  const yFor = (value: number) => padY + (1 - value / max) * (height - padY * 2);
  const line = points
    .map((point, index) => `${(padX + index * stepX).toFixed(1)},${yFor(point.balanceMinor).toFixed(1)}`)
    .join(" ");
  const targetY = yFor(points[0].targetMinor);
  const last = points[points.length - 1];

  return (
    <div>
      <svg
        viewBox={`0 0 ${width} ${height}`}
        className="h-[190px] w-full"
        role="img"
        aria-label="Projected balance against the FI target"
      >
        <defs>
          <linearGradient id="fi-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="rgb(124 143 106)" stopOpacity="0.25" />
            <stop offset="100%" stopColor="rgb(124 143 106)" stopOpacity="0" />
          </linearGradient>
        </defs>
        <polygon
          points={`${padX},${height - padY} ${line} ${(padX + (points.length - 1) * stepX).toFixed(1)},${height - padY}`}
          fill="url(#fi-fill)"
        />
        <line
          x1={padX}
          x2={width - padX}
          y1={targetY}
          y2={targetY}
          stroke="rgb(214 178 122)"
          strokeWidth="1"
          strokeDasharray="5 4"
          opacity="0.7"
        />
        <polyline
          points={line}
          fill="none"
          stroke="rgb(124 143 106)"
          strokeWidth="1.9"
          strokeLinejoin="round"
          strokeLinecap="round"
        />
      </svg>
      <div className="flex items-center justify-between text-[0.58rem] text-zinc-600">
        <span>Today</span>
        <span className="inline-flex items-center gap-1.5">
          <span className="h-[1px] w-4 border-t border-dashed border-champagne-500/80" /> FI target
        </span>
        <span>
          Year {last.year} · {last.balance}
        </span>
      </div>
    </div>
  );
}

export function FiPage() {
  const { app, settings } = useBootstrap();
  const assumptions = (settings.fi_assumptions ?? {}) as Record<string, unknown>;

  const [form, setForm] = useState<FiInput>(() => ({
    annualExpenses: "",
    withdrawalRate: numberSetting(assumptions.withdrawal_rate_pct, "4"),
    currentInvested: "",
    monthlyContribution: "",
    expectedReturn: numberSetting(assumptions.annual_return_pct, "6"),
    inflation: numberSetting(assumptions.inflation_pct, "2.5"),
    yearsToRetirement: null,
  }));
  const [report, setReport] = useState<FiReport | null>(null);
  const [submitted, setSubmitted] = useState<FiInput | null>(null);

  const mutation = useMutation({
    mutationFn: (input: FiInput) => api.fiCalculate(input),
    onSuccess: (result, input) => {
      setReport(result);
      setSubmitted(input);
    },
  });

  const update = (patch: Partial<FiInput>) => setForm((current) => ({ ...current, ...patch }));

  return (
    <div className="space-y-4 rawjan-enter">
      <div>
        <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
          Independence, with assumptions shown
        </div>
        <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">FI calculator</h2>
        <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
          The target is annual expenses divided by a withdrawal rate. Projections use the real return — expected
          return reduced by inflation — and contributions are assumed to stay level in today's money. Every
          assumption is visible and nothing is written to your ledger.
        </p>
      </div>

      <div className="grid gap-3 lg:grid-cols-[0.95fr_1.05fr]">
        <Panel className="p-5">
          <SectionLabel>Assumptions</SectionLabel>
          <div className="mt-3 grid gap-3 sm:grid-cols-2">
            <Field label="Annual expenses" hint={`In ${app.primaryCurrency}`} className="sm:col-span-2">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="e.g. 120000"
                value={form.annualExpenses}
                onChange={(event) => update({ annualExpenses: event.target.value })}
              />
            </Field>
            <Field label="Withdrawal rate %" hint="Share of the portfolio withdrawn each year">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                value={form.withdrawalRate}
                onChange={(event) => update({ withdrawalRate: event.target.value })}
              />
            </Field>
            <Field label="Expected return %" hint="Nominal annual return">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                value={form.expectedReturn}
                onChange={(event) => update({ expectedReturn: event.target.value })}
              />
            </Field>
            <Field label="Inflation %" hint="Assumed constant">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                value={form.inflation}
                onChange={(event) => update({ inflation: event.target.value })}
              />
            </Field>
            <Field label="Years to retirement" hint="Optional — enables coast number">
              <input
                className="rawjan-input font-mono"
                inputMode="numeric"
                placeholder="e.g. 20"
                value={form.yearsToRetirement === null ? "" : String(form.yearsToRetirement)}
                onChange={(event) => {
                  const raw = event.target.value.trim();
                  update({
                    yearsToRetirement:
                      raw === "" || !Number.isFinite(Number(raw)) ? null : Number(raw),
                  });
                }}
              />
            </Field>
            <Field label="Current invested" hint="Manual figure, not a market value">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0"
                value={form.currentInvested}
                onChange={(event) => update({ currentInvested: event.target.value })}
              />
            </Field>
            <Field label="Monthly contribution" hint="Assumed level, end of month">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0"
                value={form.monthlyContribution}
                onChange={(event) => update({ monthlyContribution: event.target.value })}
              />
            </Field>
          </div>

          {mutation.isError ? (
            <div className="mt-3">
              <Alert tone="error" title="Could not calculate">
                {errorMessage(mutation.error)}
              </Alert>
            </div>
          ) : null}

          <div className="mt-4 flex items-center justify-between gap-3">
            <div className="text-[0.6rem] leading-relaxed text-zinc-600">
              Defaults come from saved FI assumptions in Settings. Values here are not saved automatically.
            </div>
            <Button
              variant="primary"
              size="sm"
              loading={mutation.isPending}
              onClick={() => mutation.mutate(form)}
            >
              <Calculator className="h-3.5 w-3.5" /> Calculate
            </Button>
          </div>
        </Panel>

        <Panel className="p-5">
          {mutation.isPending ? (
            <div className="flex h-full items-center justify-center py-16 text-zinc-500">
              <Spinner className="h-5 w-5" />
            </div>
          ) : !report ? (
            <EmptyState
              className="border-0 py-12"
              icon={<Sparkles className="h-5 w-5" />}
              title="No projection yet"
              description="Enter annual expenses and a withdrawal rate, then calculate. Results appear here with their assumptions attached."
            />
          ) : (
            <div className="space-y-4">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <SectionLabel>FI number</SectionLabel>
                  <div className="mt-1.5 font-display text-[1.9rem] leading-none text-zinc-50">
                    {money(report.fiNumber, report.currency)}
                  </div>
                  <div className="mt-1 text-[0.62rem] text-zinc-600">
                    {report.annualExpenses} annual expenses at {submitted?.withdrawalRate ?? "—"}%
                  </div>
                </div>
                <Badge tone={report.progressPercent >= 100 ? "green" : "champagne"}>
                  {report.progressPercent}% funded
                </Badge>
              </div>

              <div className="h-1.5 overflow-hidden rounded-full bg-white/[0.05]">
                <div
                  className={clsx(
                    "h-full rounded-full",
                    report.progressPercent >= 100 ? "bg-sage-500" : "bg-champagne-500",
                  )}
                  style={{ width: `${Math.min(100, Math.max(2, report.progressPercent))}%` }}
                />
              </div>

              <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                <Stat label="Remaining" value={money(report.remaining, report.currency)} />
                <Stat
                  label="Time to FI"
                  value={
                    report.monthsToFi === null
                      ? "Beyond 100 years"
                      : report.monthsToFi === 0
                        ? "Reached"
                        : `${report.yearsToFi ?? "—"} yrs`
                  }
                />
                <Stat label="Projected date" value={report.fiDate ? formatDate(report.fiDate) : "—"} />
                <Stat label="Real return" value={`${report.realReturnPercent}%`} />
                <Stat
                  label="Coast number"
                  value={report.coastNumber ? money(report.coastNumber, report.currency) : "—"}
                />
                <Stat
                  label="At retirement"
                  value={report.projectedAtRetirement ? money(report.projectedAtRetirement, report.currency) : "—"}
                />
              </div>

              <div className="border-t border-white/[0.05] pt-3">
                <div className="flex items-center justify-between">
                  <SectionLabel>Projection</SectionLabel>
                  <span className="text-[0.58rem] text-zinc-600">
                    {report.projection.length} yearly points
                  </span>
                </div>
                <div className="mt-2">
                  <ProjectionChart report={report} />
                </div>
              </div>

              <ul className="space-y-1.5 border-t border-white/[0.05] pt-3 text-[0.62rem] leading-relaxed text-zinc-600">
                {report.notes.map((note) => (
                  <li key={note} className="flex gap-2">
                    <span className="mt-1.5 h-1 w-1 shrink-0 rounded-full bg-zinc-700" />
                    <span>{note}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </Panel>
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-white/[0.06] bg-black/20 px-3 py-2">
      <div className="text-[0.54rem] uppercase tracking-[0.14em] text-zinc-600">{label}</div>
      <div className="mt-0.5 truncate font-mono text-[0.68rem] text-zinc-300">{value}</div>
    </div>
  );
}
