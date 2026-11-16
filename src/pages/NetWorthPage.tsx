import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import {
  Camera,
  HandCoins,
  Landmark,
  LineChart,
  Sparkles,
  Trash2,
  Wallet,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { NetWorthSnapshot } from "@/lib/api";
import { formatDate, money } from "@/lib/display";
import { useToast } from "@/components/Toast";
import { Alert, Badge, Button, EmptyState, Panel, SectionLabel, Spinner } from "@/components/ui";

interface CompositionRow {
  key: string;
  label: string;
  minor: number;
  display: string;
  percent: number;
  tone: string;
  icon: typeof Wallet;
}

function SnapshotChart({ snapshots, currency }: { snapshots: NetWorthSnapshot[]; currency: string }) {
  const series = useMemo(
    () =>
      snapshots
        .filter((snapshot) => snapshot.currency === currency)
        .slice()
        .sort((a, b) => (a.asOfDate < b.asOfDate ? -1 : a.asOfDate > b.asOfDate ? 1 : 0)),
    [snapshots, currency],
  );

  if (series.length < 2) {
    return (
      <div className="flex h-40 items-center justify-center rounded-xl border border-dashed border-white/10 text-center text-[0.68rem] text-zinc-600">
        {series.length === 1
          ? "Capture at least one more day to draw a trend line."
          : "No snapshots in this currency yet."}
      </div>
    );
  }

  const width = 600;
  const height = 150;
  const padX = 10;
  const padY = 14;
  const values = series.map((snapshot) => snapshot.netMinor);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const stepX = (width - padX * 2) / (series.length - 1);
  const points = series.map((snapshot, index) => {
    const x = padX + index * stepX;
    const y = padY + (1 - (snapshot.netMinor - min) / span) * (height - padY * 2);
    return { x, y, snapshot };
  });
  const line = points.map((point) => `${point.x.toFixed(1)},${point.y.toFixed(1)}`).join(" ");
  const area = `${padX},${height - padY} ${line} ${(padX + (series.length - 1) * stepX).toFixed(1)},${height - padY}`;
  const first = series[0];
  const last = series[series.length - 1];

  return (
    <div>
      <svg
        viewBox={`0 0 ${width} ${height}`}
        className="h-40 w-full"
        role="img"
        aria-label={`Net worth trend across ${series.length} snapshots`}
      >
        <defs>
          <linearGradient id="networth-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="rgb(214 178 122)" stopOpacity="0.28" />
            <stop offset="100%" stopColor="rgb(214 178 122)" stopOpacity="0" />
          </linearGradient>
        </defs>
        <polygon points={area} fill="url(#networth-fill)" />
        <polyline
          points={line}
          fill="none"
          stroke="rgb(214 178 122)"
          strokeWidth="1.8"
          strokeLinejoin="round"
          strokeLinecap="round"
        />
        {points.map((point) => (
          <circle key={point.snapshot.id} cx={point.x} cy={point.y} r="2.4" fill="rgb(240 214 168)" />
        ))}
      </svg>
      <div className="mt-1 flex items-center justify-between text-[0.6rem] text-zinc-600">
        <span>{formatDate(first.asOfDate)}</span>
        <span className="text-zinc-500">
          {last.net} {last.currency}
        </span>
        <span>{formatDate(last.asOfDate)}</span>
      </div>
    </div>
  );
}

export function NetWorthPage() {
  const queryClient = useQueryClient();
  const toast = useToast();

  const overviewQuery = useQuery({
    queryKey: ["net-worth-overview"],
    queryFn: () => api.netWorthOverview(),
  });

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["net-worth-overview"] }),
      queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
    ]);
  };

  const captureMutation = useMutation({
    mutationFn: () => api.captureNetWorthSnapshot(),
    onSuccess: async (snapshot) => {
      await refresh();
      toast.success(
        "Snapshot captured",
        `${snapshot.net} ${snapshot.currency} recorded for ${formatDate(snapshot.asOfDate)}. One snapshot is kept per day.`,
      );
    },
    onError: (error) => toast.error("Snapshot not captured", errorMessage(error)),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.deleteNetWorthSnapshot(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Snapshot deleted", "The recorded history no longer includes that day.");
    },
    onError: (error) => toast.error("Snapshot not deleted", errorMessage(error)),
  });

  const overview = overviewQuery.data;
  const current = overview?.current;
  const snapshots = overview?.snapshots ?? [];

  const composition: CompositionRow[] = current
    ? [
        {
          key: "cash",
          label: "Liquid cash",
          minor: current.cashMinor,
          display: current.cash,
          percent: current.cashPercent,
          tone: "bg-sage-500/80",
          icon: Wallet,
        },
        {
          key: "invested",
          label: "Investments",
          minor: current.investedMinor,
          display: current.invested,
          percent: current.investedPercent,
          tone: "bg-lily-500/80",
          icon: LineChart,
        },
        ...(current.receivablesMinor > 0
          ? [
              {
                key: "receivables",
                label: "Money lent",
                minor: current.receivablesMinor,
                display: current.receivables,
                percent: current.receivablesPercent,
                tone: "bg-champagne-300/70",
                icon: HandCoins,
              },
            ]
          : []),
        {
          key: "other",
          label: "Other assets",
          minor: current.otherAssetsMinor,
          display: current.otherAssets,
          percent: current.otherAssetsPercent,
          tone: "bg-champagne-500/70",
          icon: Sparkles,
        },
      ]
    : [];

  const latest = overview?.latestSnapshot ?? null;

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Position, measured not guessed
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Net worth</h2>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
            Everything here comes from recorded balances. Cash and other assets use account ledgers, linked debt uses
            its liability record, and investment accounts use the latest manual valuation when one exists. Snapshots are
            taken only when you ask for them.
          </p>
        </div>
        <Button
          size="sm"
          variant="primary"
          loading={captureMutation.isPending}
          onClick={() => captureMutation.mutate()}
        >
          <Camera className="h-3.5 w-3.5" /> Capture today
        </Button>
      </div>

      {overviewQuery.isError ? (
        <Alert tone="error" title="Could not load net worth">
          {errorMessage(overviewQuery.error)}
        </Alert>
      ) : null}

      {overviewQuery.isError ? null : overviewQuery.isLoading || !current ? (
        <div className="flex items-center justify-center py-16 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : (
        <>
          <div className="grid gap-3 lg:grid-cols-[1.15fr_1fr]">
            <Panel className="p-5">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <SectionLabel>Current net worth</SectionLabel>
                  <div className="mt-2 font-display text-[2.1rem] leading-none text-zinc-50">
                    {money(current.net, current.currency)}
                  </div>
                  <div className="mt-1.5 text-[0.65rem] text-zinc-600">
                    As of {formatDate(current.asOfDate)} · {current.accountsIncluded} active account
                    {current.accountsIncluded === 1 ? "" : "s"} included
                  </div>
                </div>
                <Badge tone={current.netMinor >= 0 ? "green" : "red"}>
                  {current.netMinor >= 0 ? "Positive position" : "Negative position"}
                </Badge>
              </div>

              <div className="mt-5 grid grid-cols-2 gap-2">
                <div className="rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-3">
                  <div className="text-[0.56rem] uppercase tracking-[0.16em] text-zinc-600">Assets</div>
                  <div className="mt-1 font-display text-lg text-zinc-100">
                    {money(current.assets, current.currency)}
                  </div>
                </div>
                <div className="rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-3">
                  <div className="text-[0.56rem] uppercase tracking-[0.16em] text-zinc-600">Liabilities</div>
                  <div className="mt-1 font-display text-lg text-zinc-100">
                    {money(current.liabilities, current.currency)}
                  </div>
                </div>
              </div>

              <div className="mt-4 flex flex-wrap items-center justify-between gap-2 rounded-xl border border-white/[0.06] bg-white/[0.015] px-3.5 py-3">
                <div className="text-[0.62rem] text-zinc-500">
                  {latest ? `Change since ${formatDate(latest.asOfDate)}` : "No snapshot yet"}
                </div>
                {overview?.change !== null && overview?.change !== undefined ? (
                  <div
                    className={clsx(
                      "font-mono text-[0.8rem]",
                      (overview.changeMinor ?? 0) > 0
                        ? "text-sage-400"
                        : (overview.changeMinor ?? 0) < 0
                          ? "text-lily-300"
                          : "text-zinc-400",
                    )}
                  >
                    {(overview.changeMinor ?? 0) > 0 ? "+" : ""}
                    {money(overview.change, current.currency)}
                  </div>
                ) : (
                  <div className="text-[0.62rem] text-zinc-600">
                    {latest ? "Latest snapshot uses a different currency" : "Capture a snapshot to start tracking"}
                  </div>
                )}
              </div>

              {current.missingRates.length > 0 ? (
                <div className="mt-4">
                  <Alert tone="warning" title="Some balances were left out">
                    No exchange rate is stored for {current.missingRates.join(", ")}. Accounts in those currencies are
                    not included, so this total is incomplete. Add a rate in Settings.
                  </Alert>
                </div>
              ) : null}
            </Panel>

            <Panel className="p-5">
              <div className="flex items-center justify-between">
                <SectionLabel>Composition</SectionLabel>
                <Badge tone="muted">{current.currency}</Badge>
              </div>
              <div className="mt-3 space-y-3">
                {composition.map((row) => {
                  const Icon = row.icon;
                  return (
                    <div key={row.key}>
                      <div className="flex items-center justify-between text-[0.68rem]">
                        <span className="inline-flex items-center gap-1.5 text-zinc-400">
                          <Icon className="h-3 w-3 text-zinc-600" />
                          {row.label}
                        </span>
                        <span className="font-mono text-[0.66rem] text-zinc-500">
                          {money(row.display, current.currency)}
                        </span>
                      </div>
                      <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-white/[0.05]">
                        <div
                          className={clsx("h-full rounded-full", row.tone)}
                          style={{ width: `${Math.min(100, row.percent)}%` }}
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
              <div className="mt-4 border-t border-white/[0.05] pt-3 text-[0.63rem] leading-relaxed text-zinc-600">
                Investment accounts use the latest manual valuation on or before today; without one, the ledger
                balance is used. No market feed is connected, so valuations are only as fresh as you make them.
              </div>
            </Panel>
          </div>

          <div className="grid gap-3 lg:grid-cols-[1fr_1.15fr]">
            <Panel className="p-5">
              <div className="flex items-center justify-between">
                <SectionLabel>Recorded trend</SectionLabel>
                <Badge tone="champagne">{snapshots.length} snapshot{snapshots.length === 1 ? "" : "s"}</Badge>
              </div>
              <div className="mt-3">
                <SnapshotChart snapshots={snapshots} currency={current.currency} />
              </div>
              <div className="mt-3 text-[0.63rem] leading-relaxed text-zinc-600">
                The line plots only snapshots stored in {current.currency}. Snapshots in another currency are kept in the
                history but excluded here rather than converted after the fact.
              </div>
            </Panel>

            <Panel className="p-5">
              <div className="flex items-center justify-between">
                <SectionLabel>Snapshot history</SectionLabel>
                <Button
                  size="sm"
                  variant="ghost"
                  loading={captureMutation.isPending}
                  onClick={() => captureMutation.mutate()}
                >
                  <Camera className="h-3 w-3" /> Capture
                </Button>
              </div>

              {snapshots.length === 0 ? (
                <EmptyState
                  icon={<Landmark className="h-5 w-5" />}
                  title="No snapshots yet"
                  description="A snapshot stores the totals for one day. Capture when you want a fixed point to compare against later."
                  action={
                    <Button
                      size="sm"
                      variant="primary"
                      loading={captureMutation.isPending}
                      onClick={() => captureMutation.mutate()}
                    >
                      <Camera className="h-3.5 w-3.5" /> Capture today
                    </Button>
                  }
                />
              ) : (
                <div className="mt-3 overflow-x-auto">
                  <table className="w-full border-collapse text-left text-[0.68rem]">
                    <thead>
                      <tr className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">
                        <th scope="col" className="py-1.5 pr-2 font-medium">Date</th>
                        <th scope="col" className="py-1.5 pr-2 text-right font-medium">Assets</th>
                        <th scope="col" className="py-1.5 pr-2 text-right font-medium">Liabilities</th>
                        <th scope="col" className="py-1.5 pr-2 text-right font-medium">Net</th>
                        <th scope="col" className="py-1.5 pl-1 text-right font-medium"> </th>
                      </tr>
                    </thead>
                    <tbody>
                      {snapshots.map((snapshot) => (
                        <tr
                          key={snapshot.id}
                          className="border-t border-white/[0.05] text-zinc-400 transition hover:bg-white/[0.02]"
                        >
                          <td className="py-1.5 pr-2 whitespace-nowrap text-zinc-300">
                            {formatDate(snapshot.asOfDate)}
                          </td>
                          <td className="py-1.5 pr-2 text-right font-mono whitespace-nowrap">
                            {snapshot.assets}
                          </td>
                          <td className="py-1.5 pr-2 text-right font-mono whitespace-nowrap">
                            {snapshot.liabilities}
                          </td>
                          <td className="py-1.5 pr-2 text-right font-mono whitespace-nowrap text-zinc-100">
                            {snapshot.net} <span className="text-[0.58rem] text-zinc-600">{snapshot.currency}</span>
                          </td>
                          <td className="py-1.5 pl-1 text-right">
                            <button
                              type="button"
                              aria-label={`Delete snapshot from ${formatDate(snapshot.asOfDate)}`}
                              className="rounded-md p-1 text-zinc-600 transition hover:text-lily-300"
                              disabled={deleteMutation.isPending}
                              onClick={() => {
                                if (
                                  window.confirm(
                                    `Delete the snapshot from ${formatDate(snapshot.asOfDate)}? Current balances are not affected.`,
                                  )
                                ) {
                                  deleteMutation.mutate(snapshot.id);
                                }
                              }}
                            >
                              <Trash2 className="h-3 w-3" />
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}

              <div className="mt-3 border-t border-white/[0.05] pt-3 text-[0.63rem] leading-relaxed text-zinc-600">
                Snapshots are manual and idempotent per day: capturing again on the same date updates that day's stored
                totals. Deleting a snapshot never changes an account balance or transaction.
              </div>
            </Panel>
          </div>
        </>
      )}
    </div>
  );
}
