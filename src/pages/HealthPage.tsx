import { useQuery } from "@tanstack/react-query";
import clsx from "clsx";
import { Activity, Flame, ShieldAlert, ShieldCheck } from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import { money } from "@/lib/display";
import { Alert, Badge, EmptyState, Panel, SectionLabel, Spinner } from "@/components/ui";
import type { FinancialHealthComponent } from "@/lib/api";

function scoreTone(score: number): { text: string; stroke: string } {
  if (score >= 85) return { text: "text-sage-400", stroke: "rgb(124 143 106)" };
  if (score >= 70) return { text: "text-champagne-300", stroke: "rgb(214 178 122)" };
  if (score >= 55) return { text: "text-amber-300", stroke: "rgb(251 191 36)" };
  return { text: "text-lily-300", stroke: "rgb(195 54 77)" };
}

function ScoreRing({ score }: { score: number }) {
  const radius = 52;
  const circumference = 2 * Math.PI * radius;
  const clamped = Math.min(100, Math.max(0, score));
  const tone = scoreTone(clamped);
  return (
    <div className="relative flex h-[132px] w-[132px] items-center justify-center">
      <svg viewBox="0 0 132 132" className="h-full w-full -rotate-90" role="img" aria-label={`Score ${score} of 100`}>
        <circle cx="66" cy="66" r={radius} fill="none" stroke="rgba(255,255,255,0.06)" strokeWidth="9" />
        <circle
          cx="66"
          cy="66"
          r={radius}
          fill="none"
          stroke={tone.stroke}
          strokeWidth="9"
          strokeLinecap="round"
          strokeDasharray={`${(clamped / 100) * circumference} ${circumference}`}
        />
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className={clsx("font-display text-[2.2rem] leading-none", tone.text)}>{score}</span>
        <span className="mt-0.5 text-[0.55rem] uppercase tracking-[0.16em] text-zinc-600">of 100</span>
      </div>
    </div>
  );
}

function ComponentRow({ component }: { component: FinancialHealthComponent }) {
  const available = component.available && component.score !== null;
  const score = component.score ?? 0;
  const barTone = !available
    ? "bg-zinc-700/60"
    : score >= 75
      ? "bg-sage-500/80"
      : score >= 50
        ? "bg-champagne-500/80"
        : "bg-lily-500/80";
  return (
    <div className="rounded-xl border border-white/[0.06] bg-white/[0.015] p-3.5">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[0.76rem] text-zinc-200">{component.label}</span>
            <span className="text-[0.56rem] uppercase tracking-[0.12em] text-zinc-600">
              weight {component.weight}
            </span>
          </div>
          <p className="mt-1 text-[0.66rem] leading-relaxed text-zinc-500">{component.detail}</p>
        </div>
        <div className="shrink-0 text-right">
          {available ? (
            <>
              <div className="font-mono text-sm text-zinc-200">{score}</div>
              {component.metric ? (
                <div className="text-[0.58rem] text-zinc-600">{component.metric}</div>
              ) : null}
            </>
          ) : (
            <Badge tone="muted">Not scored</Badge>
          )}
        </div>
      </div>
      <div className="mt-2.5 h-1 overflow-hidden rounded-full bg-white/[0.05]">
        <div
          className={clsx("h-full rounded-full transition-all", barTone)}
          style={{ width: `${available ? Math.max(score, 2) : 0}%` }}
        />
      </div>
    </div>
  );
}

export function HealthPage() {
  const reportQuery = useQuery({
    queryKey: ["financial-health"],
    queryFn: () => api.financialHealth(),
  });

  const report = reportQuery.data;

  return (
    <div className="space-y-4 rawjan-enter">
      <div>
        <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
          A transparent summary
        </div>
        <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Financial health</h2>
        <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
          A deterministic score from recorded data. Components with no underlying data are excluded and the
          remaining weights are renormalised, so missing information never silently counts as a zero. This is not
          a credit score, a rating or advice.
        </p>
      </div>

      {reportQuery.isError ? (
        <Alert tone="error" title="Could not load the health report">
          {errorMessage(reportQuery.error)}
        </Alert>
      ) : null}

      {reportQuery.isError ? null : reportQuery.isLoading || !report ? (
        <div className="flex items-center justify-center py-16 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : (
        <>
          <div className="grid gap-3 lg:grid-cols-[0.9fr_1.1fr]">
            <Panel className="flex flex-col items-center justify-center p-6">
              <ScoreRing score={report.score} />
              <div className="mt-3 text-center">
                <div className="font-display text-xl text-zinc-100">
                  {report.label}{" "}
                  <span className="text-[0.7rem] font-sans uppercase tracking-[0.16em] text-zinc-500">
                    grade {report.grade}
                  </span>
                </div>
                <div className="mt-1 text-[0.62rem] text-zinc-600">
                  Weighted across the components that could be scored
                </div>
              </div>
              <div className="mt-4 grid w-full grid-cols-2 gap-2">
                <Figure label="Liquid cash" value={money(report.cash, report.currency)} />
                <Figure label="Debt" value={money(report.debt, report.currency)} />
                <Figure label="Assets" value={money(report.assets, report.currency)} />
                <Figure label="Invested" value={money(report.invested, report.currency)} />
              </div>
            </Panel>

            <Panel className="p-5">
              <SectionLabel>What is being measured</SectionLabel>
              <div className="mt-3 grid gap-2 sm:grid-cols-2">
                <Metric
                  label="Savings rate"
                  value={report.savingsRate === null ? "Not measurable" : `${report.savingsRate}%`}
                  hint="Last three complete calendar months"
                />
                <Metric
                  label="Emergency coverage"
                  value={
                    report.emergencyMonths === null ? "Not measurable" : `${report.emergencyMonths} months`
                  }
                  hint="Liquid cash ÷ average monthly expense"
                />
                <Metric
                  label="Debt to assets"
                  value={report.debtRatio === null ? "Not measurable" : `${report.debtRatio}%`}
                  hint="Liabilities ÷ assets"
                />
                <Metric
                  label="Invested share"
                  value={report.investedShare === null ? "Not measurable" : `${report.investedShare}%`}
                  hint="Investments ÷ assets"
                />
                <Metric
                  label="Budgets within"
                  value={
                    report.budgetCount > 0
                      ? `${report.budgetsWithin} of ${report.budgetCount}`
                      : "No active budgets"
                  }
                  hint="Active budgets at or under allocation"
                />
                <Metric
                  label="Net-worth trend"
                  value={
                    report.netWorthTrend === null ? "Not measurable" : `${report.netWorthTrend}%`
                  }
                  hint="Earliest to latest snapshot, 30+ days apart"
                />
              </div>
              <div className="mt-4 flex flex-wrap items-center gap-2 border-t border-white/[0.05] pt-3">
                {report.averageMonthlyExpense ? (
                  <Badge tone="muted">Avg. expense {report.averageMonthlyExpense}</Badge>
                ) : (
                  <Badge tone="muted">No expense history</Badge>
                )}
                <Badge tone="champagne">{report.currency}</Badge>
              </div>
            </Panel>
          </div>

          <Panel className="p-5">
            <div className="flex items-center justify-between">
              <SectionLabel>Components</SectionLabel>
              <span className="text-[0.6rem] text-zinc-600">
                {report.components.filter((component) => component.available).length} of{" "}
                {report.components.length} scored
              </span>
            </div>
            <div className="mt-3 grid gap-2.5 lg:grid-cols-2">
              {report.components.map((component) => (
                <ComponentRow key={component.key} component={component} />
              ))}
            </div>
          </Panel>

          <div className="grid gap-3 lg:grid-cols-2">
            <Panel className="p-5">
              <div className="flex items-center gap-2">
                <ShieldCheck className="h-4 w-4 text-sage-500" />
                <SectionLabel>How to read this</SectionLabel>
              </div>
              <ul className="mt-3 space-y-2 text-[0.7rem] leading-relaxed text-zinc-500">
                {report.notes.map((note) => (
                  <li key={note} className="flex gap-2">
                    <span className="mt-1.5 h-1 w-1 shrink-0 rounded-full bg-zinc-700" />
                    <span>{note}</span>
                  </li>
                ))}
              </ul>
            </Panel>

            <Panel className="p-5">
              <div className="flex items-center gap-2">
                <ShieldAlert className="h-4 w-4 text-champagne-400" />
                <SectionLabel>Limits</SectionLabel>
              </div>
              <div className="mt-3 space-y-2 text-[0.7rem] leading-relaxed text-zinc-500">
                <p>
                  The score reflects only what is recorded here. Cash held outside tracked accounts, informal
                  debts and future obligations are invisible to it.
                </p>
                <p>
                  No market prices are connected. Investment values come from manual valuations, so the invested
                  share is only as current as the last valuation you saved.
                </p>
                <p>
                  Use it to notice patterns, not to compare yourself with anyone else. It is not financial advice.
                </p>
              </div>
            </Panel>
          </div>
        </>
      )}

      {report && report.components.every((component) => !component.available) ? (
        <EmptyState
          icon={<Flame className="h-5 w-5" />}
          title="Nothing to score yet"
          description="Record transactions, accounts and a snapshot or two. Components without data are left unscored."
        />
      ) : null}

      <div className="flex items-center gap-1.5 text-[0.6rem] text-zinc-700">
        <Activity className="h-3 w-3" />
        Recomputed on each visit from the local database. Nothing is cached to disk.
      </div>
    </div>
  );
}

function Figure({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-white/[0.06] bg-black/20 px-3 py-2">
      <div className="text-[0.54rem] uppercase tracking-[0.14em] text-zinc-600">{label}</div>
      <div className="mt-0.5 truncate font-mono text-[0.68rem] text-zinc-300">{value}</div>
    </div>
  );
}

function Metric({ label, value, hint }: { label: string; value: string; hint: string }) {
  return (
    <div className="rounded-lg border border-white/[0.05] bg-white/[0.015] px-3 py-2.5">
      <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">{label}</div>
      <div className="mt-0.5 text-[0.8rem] text-zinc-200">{value}</div>
      <div className="mt-0.5 text-[0.56rem] leading-relaxed text-zinc-600">{hint}</div>
    </div>
  );
}
