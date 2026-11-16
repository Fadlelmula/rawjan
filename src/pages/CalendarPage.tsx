import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import clsx from "clsx";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { CalendarDay } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { formatDate, money } from "@/lib/display";
import { Alert, Badge, Button, EmptyState, Panel, SectionLabel, Spinner } from "@/components/ui";

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function shiftMonth(month: string, delta: number): string {
  const [yearRaw, monthRaw] = month.split("-");
  const year = Number(yearRaw);
  const monthIndex = Number(monthRaw) - 1;
  if (!Number.isFinite(year) || !Number.isFinite(monthIndex)) return month;
  const date = new Date(year, monthIndex + delta, 1);
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
}

function monthHeading(month: string): string {
  const [yearRaw, monthRaw] = month.split("-");
  const date = new Date(Number(yearRaw), Number(monthRaw) - 1, 1);
  if (Number.isNaN(date.getTime())) return month;
  return date.toLocaleDateString("en-US", { month: "long", year: "numeric" });
}

export function CalendarPage() {
  const { app } = useBootstrap();
  const [month, setMonth] = useState(() => app.today.slice(0, 7));
  const [selected, setSelected] = useState<string | null>(null);

  const calendarQuery = useQuery({
    queryKey: ["calendar", month],
    queryFn: () => api.monthCalendar(month),
  });

  const calendar = calendarQuery.data;

  const activeByDate = useMemo(() => {
    const map = new Map<string, CalendarDay>();
    calendar?.days.forEach((day) => map.set(day.date, day));
    return map;
  }, [calendar]);

  const cells = useMemo(() => {
    if (!calendar) return [];
    const leading = Array.from({ length: calendar.firstWeekday }, () => null);
    const days = Array.from({ length: calendar.daysInMonth }, (_, index) => index + 1);
    return [...leading, ...days];
  }, [calendar]);

  const selectedDay = selected ? activeByDate.get(selected) ?? null : null;
  const selectedIso = selected;

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Timing, without forecasts
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Calendar</h2>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
            Only actual, effective activity appears here. Pending, draft and reversed rows are excluded, and no
            scheduled or projected cash flow is drawn — this calendar shows what happened, not what might.
          </p>
        </div>
        <div className="flex items-center gap-1.5">
          <Button size="sm" variant="ghost" onClick={() => setMonth(shiftMonth(month, -1))} aria-label="Previous month">
            <ChevronLeft className="h-3.5 w-3.5" />
          </Button>
          <div className="min-w-[9rem] text-center font-display text-sm text-zinc-200">
            {monthHeading(month)}
          </div>
          <Button size="sm" variant="ghost" onClick={() => setMonth(shiftMonth(month, 1))} aria-label="Next month">
            <ChevronRight className="h-3.5 w-3.5" />
          </Button>
          <Button size="sm" variant="secondary" onClick={() => setMonth(app.today.slice(0, 7))}>
            Today
          </Button>
        </div>
      </div>

      {calendarQuery.isError ? (
        <Alert tone="error" title="Could not load the calendar">
          {errorMessage(calendarQuery.error)}
        </Alert>
      ) : null}

      {calendarQuery.isLoading || !calendar ? (
        <div className="flex items-center justify-center py-16 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : (
        <div className="grid gap-3 xl:grid-cols-[1.6fr_1fr]">
          <Panel className="p-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <SectionLabel>{calendar.label}</SectionLabel>
              <div className="flex flex-wrap items-center gap-2 text-[0.6rem]">
                <Badge tone="green">{calendar.income} in</Badge>
                <Badge tone="red">{calendar.expense} out</Badge>
                <Badge tone="muted">
                  {calendar.activeDays} active day{calendar.activeDays === 1 ? "" : "s"}
                </Badge>
              </div>
            </div>

            <div className="mt-4 grid grid-cols-7 gap-1">
              {WEEKDAYS.map((label) => (
                <div
                  key={label}
                  className="pb-1 text-center text-[0.56rem] font-semibold uppercase tracking-[0.14em] text-zinc-600"
                >
                  {label}
                </div>
              ))}
              {cells.map((day, index) => {
                if (day === null) {
                  return <div key={`blank-${index}`} className="aspect-square rounded-lg bg-transparent" />;
                }
                const iso = `${calendar.month}-${String(day).padStart(2, "0")}`;
                const entry = activeByDate.get(iso);
                const isToday = iso === app.today;
                const isSelected = selectedIso === iso;
                return (
                  <button
                    key={iso}
                    type="button"
                    onClick={() => setSelected(entry ? iso : null)}
                    className={clsx(
                      "group relative flex aspect-square flex-col items-center justify-start gap-0.5 rounded-lg border p-1 text-center transition",
                      entry
                        ? "border-white/[0.08] bg-white/[0.025] hover:border-lily-500/30 hover:bg-white/[0.05]"
                        : "border-transparent text-zinc-700",
                      isSelected && "border-lily-500/50 bg-lily-600/10",
                      isToday && !isSelected && "border-champagne-500/40",
                    )}
                    title={entry ? `${entry.transactionCount} transaction(s)` : "No effective activity"}
                  >
                    <span
                      className={clsx(
                        "text-[0.68rem] leading-none",
                        isToday ? "text-champagne-300" : entry ? "text-zinc-200" : "text-zinc-600",
                      )}
                    >
                      {day}
                    </span>
                    {entry ? (
                      <span className="mt-0.5 flex flex-col items-center gap-[1px]">
                        {entry.expenseMinor > 0 ? (
                          <span className="max-w-full truncate font-mono text-[0.5rem] leading-tight text-lily-300/90">
                            −{entry.expense}
                          </span>
                        ) : null}
                        {entry.incomeMinor > 0 ? (
                          <span className="max-w-full truncate font-mono text-[0.5rem] leading-tight text-sage-400/90">
                            +{entry.income}
                          </span>
                        ) : null}
                      </span>
                    ) : null}
                  </button>
                );
              })}
            </div>

            <div className="mt-3 border-t border-white/[0.05] pt-3 text-[0.62rem] leading-relaxed text-zinc-600">
              Amounts are converted to {calendar.currency} with stored exchange-rate snapshots. Days without a
              marker have no posted or reconciled activity.
            </div>
          </Panel>

          <div className="space-y-3">
            <Panel className="p-5">
              <SectionLabel>Month totals</SectionLabel>
              <div className="mt-3 space-y-2.5">
                <TotalRow label="Income" value={money(calendar.income, calendar.currency)} tone="text-sage-400" />
                <TotalRow label="Expense" value={money(calendar.expense, calendar.currency)} tone="text-zinc-100" />
                <TotalRow
                  label="Net"
                  value={money(calendar.net, calendar.currency)}
                  tone={calendar.netMinor >= 0 ? "text-sage-400" : "text-lily-300"}
                />
                <TotalRow
                  label="Transactions"
                  value={String(calendar.transactionCount)}
                  tone="text-zinc-300"
                />
              </div>
              <div className="mt-4 text-[0.62rem] leading-relaxed text-zinc-600">
                Period {formatDate(calendar.from)} – {formatDate(calendar.to)}.
              </div>
            </Panel>

            <Panel className="p-5">
              <div className="flex items-center justify-between">
                <SectionLabel>Selected day</SectionLabel>
                {selectedIso ? <Badge tone="muted">{formatDate(selectedIso)}</Badge> : null}
              </div>
              {!selectedIso ? (
                <EmptyState
                  className="mt-3 border-0 px-2 py-8"
                  icon={<CalendarDays className="h-5 w-5" />}
                  title="Pick an active day"
                  description="Select a day with a marker to see its recorded totals. Empty days stay empty."
                />
              ) : selectedDay ? (
                <div className="mt-3 space-y-2.5">
                  <TotalRow
                    label="Income"
                    value={money(selectedDay.income, calendar.currency)}
                    tone="text-sage-400"
                  />
                  <TotalRow
                    label="Expense"
                    value={money(selectedDay.expense, calendar.currency)}
                    tone="text-zinc-100"
                  />
                  <TotalRow
                    label="Net"
                    value={money(selectedDay.net, calendar.currency)}
                    tone={selectedDay.netMinor >= 0 ? "text-sage-400" : "text-lily-300"}
                  />
                  <TotalRow
                    label="Transactions"
                    value={String(selectedDay.transactionCount)}
                    tone="text-zinc-300"
                  />
                  <div className="pt-1 text-[0.62rem] leading-relaxed text-zinc-600">
                    This is a daily aggregate. Open Transactions to see the individual rows behind it.
                  </div>
                </div>
              ) : (
                <div className="mt-3 text-[0.7rem] text-zinc-500">
                  That day has no effective activity.
                </div>
              )}
            </Panel>
          </div>
        </div>
      )}
    </div>
  );
}

function TotalRow({ label, value, tone }: { label: string; value: string; tone: string }) {
  return (
    <div className="flex items-center justify-between gap-3 border-b border-white/[0.04] pb-2 last:border-0 last:pb-0">
      <span className="text-[0.7rem] text-zinc-500">{label}</span>
      <span className={clsx("font-mono text-[0.72rem]", tone)}>{value}</span>
    </div>
  );
}
