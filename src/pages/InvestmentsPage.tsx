import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import {
  ArrowDownRight,
  ArrowUpRight,
  ChevronDown,
  ChevronUp,
  Coins,
  Pencil,
  Plus,
  Trash2,
  TrendingUp,
  Wallet,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type {
  Instrument,
  InstrumentInput,
  Trade,
  TradeInput,
  Valuation,
  ValuationInput,
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

const ASSET_CLASSES = [
  { value: "stock", label: "Stock" },
  { value: "etf", label: "ETF" },
  { value: "bond", label: "Bond" },
  { value: "gold", label: "Gold" },
  { value: "silver", label: "Silver" },
  { value: "crypto", label: "Crypto" },
  { value: "real_estate", label: "Real estate" },
  { value: "cash_deposit", label: "Cash deposit" },
  { value: "other", label: "Other" },
];

const ASSET_LABEL: Record<string, string> = Object.fromEntries(
  ASSET_CLASSES.map((item) => [item.value, item.label]),
);

type Tab = "holdings" | "trades" | "valuations";

interface InstrumentFormState {
  symbol: string;
  name: string;
  assetClass: string;
  currencyCode: string;
  exchange: string;
  isin: string;
  notes: string;
}

interface TradeFormState {
  accountId: string;
  instrumentId: string;
  side: string;
  tradeDate: string;
  quantity: string;
  price: string;
  fee: string;
  notes: string;
}

interface ValuationFormState {
  accountId: string;
  instrumentId: string;
  asOfDate: string;
  value: string;
  sourceRef: string;
}

function toneForSign(minor: number | null): string {
  if (minor === null) return "text-zinc-500";
  if (minor < 0) return "text-lily-300";
  if (minor > 0) return "text-sage-400";
  return "text-zinc-400";
}

export function InvestmentsPage() {
  const { app, currencies, accounts } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();
  const [tab, setTab] = useState<Tab>("holdings");

  const instrumentQuery = useQuery({ queryKey: ["instruments"], queryFn: () => api.listInstruments() });
  const summaryQuery = useQuery({ queryKey: ["portfolio"], queryFn: () => api.portfolioSummary() });
  const tradesQuery = useQuery({ queryKey: ["trades"], queryFn: () => api.listTrades() });
  const valuationsQuery = useQuery({ queryKey: ["valuations"], queryFn: () => api.listValuations() });

  const investmentAccounts = useMemo(
    () => accounts.filter((account) => account.isActive && account.accountType === "investment"),
    [accounts],
  );

  const [instrumentOpen, setInstrumentOpen] = useState(false);
  const [editingInstrument, setEditingInstrument] = useState<Instrument | null>(null);
  const [instrumentForm, setInstrumentForm] = useState<InstrumentFormState>({
    symbol: "",
    name: "",
    assetClass: "etf",
    currencyCode: app.primaryCurrency,
    exchange: "",
    isin: "",
    notes: "",
  });
  const [instrumentError, setInstrumentError] = useState<string | null>(null);

  const [tradeOpen, setTradeOpen] = useState(false);
  const [editingTrade, setEditingTrade] = useState<Trade | null>(null);
  const [tradeForm, setTradeForm] = useState<TradeFormState>({
    accountId: "",
    instrumentId: "",
    side: "buy",
    tradeDate: app.today,
    quantity: "",
    price: "",
    fee: "",
    notes: "",
  });
  const [tradeError, setTradeError] = useState<string | null>(null);

  const [valuationOpen, setValuationOpen] = useState(false);
  const [editingValuation, setEditingValuation] = useState<Valuation | null>(null);
  const [valuationForm, setValuationForm] = useState<ValuationFormState>({
    accountId: "",
    instrumentId: "",
    asOfDate: app.today,
    value: "",
    sourceRef: "",
  });
  const [valuationError, setValuationError] = useState<string | null>(null);

  const [expanded, setExpanded] = useState<Record<string, boolean>>({});

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["instruments"] }),
      queryClient.invalidateQueries({ queryKey: ["portfolio"] }),
      queryClient.invalidateQueries({ queryKey: ["trades"] }),
      queryClient.invalidateQueries({ queryKey: ["valuations"] }),
      queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
    ]);
  };

  const instrumentMutation = useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: InstrumentInput }) =>
      api.saveInstrument(id, input),
    onSuccess: async () => {
      await refresh();
      toast.success(editingInstrument ? "Instrument updated" : "Instrument created");
      setInstrumentOpen(false);
      setEditingInstrument(null);
    },
    onError: (error) => setInstrumentError(errorMessage(error)),
  });

  const deleteInstrumentMutation = useMutation({
    mutationFn: (id: string) => api.deleteInstrument(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Instrument deleted");
    },
    onError: (error) => toast.error("Instrument not deleted", errorMessage(error)),
  });

  const tradeMutation = useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: TradeInput }) => api.saveTrade(id, input),
    onSuccess: async () => {
      await refresh();
      toast.success(editingTrade ? "Trade updated" : "Trade recorded", "Trades are records here and do not post to the ledger.");
      setTradeOpen(false);
      setEditingTrade(null);
    },
    onError: (error) => setTradeError(errorMessage(error)),
  });

  const deleteTradeMutation = useMutation({
    mutationFn: (id: string) => api.deleteTrade(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Trade deleted");
    },
    onError: (error) => toast.error("Trade not deleted", errorMessage(error)),
  });

  const valuationMutation = useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: ValuationInput }) =>
      api.saveValuation(id, input),
    onSuccess: async () => {
      await refresh();
      toast.success(editingValuation ? "Valuation updated" : "Valuation recorded");
      setValuationOpen(false);
      setEditingValuation(null);
    },
    onError: (error) => setValuationError(errorMessage(error)),
  });

  const deleteValuationMutation = useMutation({
    mutationFn: (id: string) => api.deleteValuation(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Valuation deleted");
    },
    onError: (error) => toast.error("Valuation not deleted", errorMessage(error)),
  });

  const instruments = instrumentQuery.data ?? [];
  const trades = tradesQuery.data ?? [];
  const valuations = valuationsQuery.data ?? [];
  const summary = summaryQuery.data;

  const openCreateInstrument = () => {
    setEditingInstrument(null);
    setInstrumentForm({
      symbol: "",
      name: "",
      assetClass: "etf",
      currencyCode: app.primaryCurrency,
      exchange: "",
      isin: "",
      notes: "",
    });
    setInstrumentError(null);
    setInstrumentOpen(true);
  };

  const openEditInstrument = (instrument: Instrument) => {
    setEditingInstrument(instrument);
    setInstrumentForm({
      symbol: instrument.symbol,
      name: instrument.name,
      assetClass: instrument.assetClass,
      currencyCode: instrument.currency,
      exchange: instrument.exchange ?? "",
      isin: instrument.isin ?? "",
      notes: instrument.notes ?? "",
    });
    setInstrumentError(null);
    setInstrumentOpen(true);
  };

  const submitInstrument = () => {
    setInstrumentError(null);
    if (!instrumentForm.symbol.trim() || !instrumentForm.name.trim()) {
      setInstrumentError("Symbol and name are required.");
      return;
    }
    instrumentMutation.mutate({
      id: editingInstrument?.id ?? null,
      input: {
        symbol: instrumentForm.symbol.trim(),
        name: instrumentForm.name.trim(),
        assetClass: instrumentForm.assetClass,
        currencyCode: instrumentForm.currencyCode,
        exchange: instrumentForm.exchange.trim() || null,
        isin: instrumentForm.isin.trim() || null,
        notes: instrumentForm.notes.trim() || null,
      },
    });
  };

  const openCreateTrade = (instrument?: Instrument) => {
    if (investmentAccounts.length === 0) {
      toast.error("No investment account", "Create an investment account before recording a trade.");
      return;
    }
    if (instruments.length === 0) {
      toast.error("No instrument", "Create an instrument before recording a trade.");
      return;
    }
    setEditingTrade(null);
    setTradeForm({
      accountId: investmentAccounts[0].id,
      instrumentId: instrument?.id ?? instruments[0].id,
      side: "buy",
      tradeDate: app.today,
      quantity: "",
      price: "",
      fee: "",
      notes: "",
    });
    setTradeError(null);
    setTradeOpen(true);
  };

  const openEditTrade = (trade: Trade) => {
    setEditingTrade(trade);
    setTradeForm({
      accountId: trade.accountId,
      instrumentId: trade.instrumentId,
      side: trade.side,
      tradeDate: trade.tradeDate,
      quantity: trade.quantity,
      price: trade.price,
      fee: trade.fee,
      notes: trade.notes ?? "",
    });
    setTradeError(null);
    setTradeOpen(true);
  };

  const submitTrade = () => {
    setTradeError(null);
    if (!tradeForm.accountId || !tradeForm.instrumentId) {
      setTradeError("Choose an account and an instrument.");
      return;
    }
    if (!tradeForm.quantity.trim() || !tradeForm.price.trim()) {
      setTradeError("Enter a quantity and a price.");
      return;
    }
    tradeMutation.mutate({
      id: editingTrade?.id ?? null,
      input: {
        accountId: tradeForm.accountId,
        instrumentId: tradeForm.instrumentId,
        side: tradeForm.side,
        tradeDate: tradeForm.tradeDate || app.today,
        quantity: tradeForm.quantity.trim(),
        price: tradeForm.price.trim(),
        fee: tradeForm.fee.trim() || null,
        notes: tradeForm.notes.trim() || null,
      },
    });
  };

  const openCreateValuation = (instrument?: Instrument) => {
    if (investmentAccounts.length === 0) {
      toast.error("No investment account", "Create an investment account before recording a valuation.");
      return;
    }
    if (instruments.length === 0) {
      toast.error("No instrument", "Create an instrument before recording a valuation.");
      return;
    }
    setEditingValuation(null);
    setValuationForm({
      accountId: investmentAccounts[0].id,
      instrumentId: instrument?.id ?? instruments[0].id,
      asOfDate: app.today,
      value: "",
      sourceRef: "",
    });
    setValuationError(null);
    setValuationOpen(true);
  };

  const openEditValuation = (valuation: Valuation) => {
    setEditingValuation(valuation);
    setValuationForm({
      accountId: valuation.accountId,
      // An account-level valuation must stay account-level when edited.
      instrumentId: valuation.instrumentId ?? "",
      asOfDate: valuation.asOfDate,
      value: valuation.value,
      sourceRef: valuation.sourceRef ?? "",
    });
    setValuationError(null);
    setValuationOpen(true);
  };

  const submitValuation = () => {
    setValuationError(null);
    if (!valuationForm.accountId) {
      setValuationError("Choose an account.");
      return;
    }
    if (!valuationForm.value.trim()) {
      setValuationError("Enter the value shown by your statement.");
      return;
    }
    valuationMutation.mutate({
      id: editingValuation?.id ?? null,
      input: {
        accountId: valuationForm.accountId,
        instrumentId: valuationForm.instrumentId || null,
        asOfDate: valuationForm.asOfDate || app.today,
        value: valuationForm.value.trim(),
        sourceRef: valuationForm.sourceRef.trim() || null,
      },
    });
  };

  const tradeInstrument = instruments.find((item) => item.id === tradeForm.instrumentId);

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Portfolio, without invented prices
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Investments</h2>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
            Instruments, trades and manual valuations are stored exactly as entered. No market feed is connected, so a
            market value exists only where you record one from a statement. Trades are tracking records in this build and
            do not post to the ledger automatically.
          </p>
        </div>
        <Button size="sm" variant="primary" onClick={openCreateInstrument}>
          <Plus className="h-3.5 w-3.5" /> New instrument
        </Button>
      </div>

      {investmentAccounts.length === 0 ? (
        <Alert tone="warning" title="No investment account">
          Create an account with type “Investment” on the Accounts page before recording trades or valuations.
        </Alert>
      ) : null}

      {summaryQuery.isError ? (
        <Alert tone="error" title="Could not load the portfolio summary">
          {errorMessage(summaryQuery.error)}
        </Alert>
      ) : null}

      {summary && summary.currencies.length > 0 ? (
        <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
          {summary.currencies.map((currency) => {
            const partial = currency.valuedHoldings < currency.holdings;
            return (
              <Panel key={currency.currency} className="p-3.5">
                <div className="flex items-center justify-between">
                  <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">
                    Portfolio · {currency.currency}
                  </div>
                  <Badge tone={partial ? "amber" : "green"}>
                    {partial
                      ? `${currency.valuedHoldings} of ${currency.holdings} valued`
                      : `${currency.holdings} valued`}
                  </Badge>
                </div>
                <div className="mt-2 grid grid-cols-2 gap-2">
                  <div>
                    <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Market value</div>
                    <div className="mt-0.5 font-display text-[1.05rem] text-zinc-100">{currency.marketValue}</div>
                    {partial ? (
                      <div className="text-[0.58rem] text-amber-300/80">Valued holdings only</div>
                    ) : null}
                  </div>
                  <div>
                    <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">Cost basis</div>
                    <div className="mt-0.5 font-mono text-[0.85rem] text-zinc-400">{currency.costBasis}</div>
                  </div>
                </div>
                <div className="mt-2 flex items-center justify-between border-t border-white/[0.05] pt-2 text-[0.64rem]">
                  <span className="text-zinc-600">Realized</span>
                  <span className={clsx("font-mono", toneForSign(currency.realizedMinor))}>{currency.realized}</span>
                </div>
              </Panel>
            );
          })}
        </div>
      ) : null}

      <div className="flex flex-wrap items-center gap-1.5 border-b border-white/[0.06] pb-2">
        {(
          [
            ["holdings", `Holdings${instruments.length ? ` (${instruments.length})` : ""}`],
            ["trades", `Trades${trades.length ? ` (${trades.length})` : ""}`],
            ["valuations", `Valuations${valuations.length ? ` (${valuations.length})` : ""}`],
          ] as [Tab, string][]
        ).map(([value, label]) => (
          <button
            key={value}
            type="button"
            onClick={() => setTab(value)}
            className={clsx(
              "rounded-lg px-3 py-1.5 text-[0.72rem] transition",
              tab === value
                ? "bg-white/[0.07] text-zinc-100"
                : "text-zinc-500 hover:bg-white/[0.03] hover:text-zinc-300",
            )}
          >
            {label}
          </button>
        ))}
        <div className="ml-auto flex items-center gap-1.5">
          <Button size="sm" variant="secondary" onClick={() => openCreateTrade()}>
            <Plus className="h-3 w-3" /> Trade
          </Button>
          <Button size="sm" variant="secondary" onClick={() => openCreateValuation()}>
            <Plus className="h-3 w-3" /> Valuation
          </Button>
        </div>
      </div>

      {instrumentQuery.isError ? (
        <Alert tone="error" title="Could not load instruments">
          {errorMessage(instrumentQuery.error)}
        </Alert>
      ) : null}

      {instrumentQuery.isLoading ? (
        <div className="flex items-center justify-center py-16 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : tab === "holdings" ? (
        instruments.length === 0 ? (
          <EmptyState
            icon={<TrendingUp className="h-5 w-5" />}
            title="No instruments yet"
            description="Add an instrument, then record buys and sells. Nothing is pre-filled and no price is fetched."
            action={
              <Button size="sm" variant="primary" onClick={openCreateInstrument}>
                <Plus className="h-3.5 w-3.5" /> Create first instrument
              </Button>
            }
          />
        ) : (
          <div className="grid gap-3 xl:grid-cols-2">
            {instruments.map((instrument) => {
              const open = expanded[instrument.id] ?? false;
              return (
                <Panel key={instrument.id} className="flex h-full flex-col p-4">
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className="truncate font-display text-[1.05rem] leading-tight text-zinc-100">
                          {instrument.symbol}
                        </h3>
                        <Badge tone="muted">{ASSET_LABEL[instrument.assetClass] ?? instrument.assetClass}</Badge>
                        <Badge tone="default">{instrument.currency}</Badge>
                      </div>
                      <div className="mt-0.5 truncate text-[0.66rem] text-zinc-500">{instrument.name}</div>
                      <div className="mt-0.5 text-[0.6rem] text-zinc-600">
                        {instrument.exchange ? `${instrument.exchange} · ` : ""}
                        {instrument.tradeCount} trade{instrument.tradeCount === 1 ? "" : "s"}
                        {instrument.isin ? ` · ISIN ${instrument.isin}` : ""}
                      </div>
                    </div>
                    <div className="flex shrink-0 items-center gap-1">
                      <Button
                        size="sm"
                        variant="ghost"
                        aria-label={`Edit ${instrument.symbol}`}
                        onClick={() => openEditInstrument(instrument)}
                      >
                        <Pencil className="h-3 w-3" />
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="text-zinc-600"
                        aria-label={`Delete ${instrument.symbol}`}
                        disabled={deleteInstrumentMutation.isPending}
                        onClick={() => {
                          if (
                            window.confirm(
                              `Delete ${instrument.symbol}? Instruments with trades or valuations cannot be deleted.`,
                            )
                          ) {
                            deleteInstrumentMutation.mutate(instrument.id);
                          }
                        }}
                      >
                        <Trash2 className="h-3 w-3" />
                      </Button>
                    </div>
                  </div>

                  {instrument.holdingCount === 0 ? (
                    <div className="mt-4 rounded-xl border border-dashed border-white/10 px-4 py-4 text-center text-[0.68rem] text-zinc-600">
                      No open holding. Record a buy to start tracking this instrument.
                    </div>
                  ) : (
                    <>
                      <div className="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-4">
                        <Metric label="Quantity" value={instrument.quantity} mono />
                        <Metric label="Cost basis" value={instrument.costBasis} mono />
                        <Metric
                          label="Market value"
                          value={instrument.marketValue ?? "No valuation"}
                          tone={instrument.marketValue ? "text-zinc-200" : "text-zinc-600"}
                          mono
                        />
                        <Metric
                          label="Unrealized"
                          value={instrument.unrealized ?? "Not comparable"}
                          tone={toneForSign(instrument.unrealizedMinor)}
                          mono
                        />
                      </div>
                      <div className="mt-2 flex items-center justify-between text-[0.6rem] text-zinc-600">
                        <span>
                          Realized{" "}
                          <span className={clsx("font-mono", toneForSign(instrument.realizedMinor))}>
                            {instrument.realized}
                          </span>
                        </span>
                        <span>
                          {instrument.valuedHoldings} of {instrument.holdingCount} holding
                          {instrument.holdingCount === 1 ? "" : "s"} valued
                        </span>
                      </div>
                    </>
                  )}

                  <div className="mt-3 border-t border-white/[0.05] pt-2">
                    <button
                      type="button"
                      className="flex w-full items-center justify-between text-[0.62rem] text-zinc-500 transition hover:text-zinc-300"
                      onClick={() => setExpanded({ ...expanded, [instrument.id]: !open })}
                    >
                      <span>
                        {instrument.holdings.length === 0
                          ? "No holdings by account"
                          : `${instrument.holdings.length} account holding${instrument.holdings.length === 1 ? "" : "s"}`}
                      </span>
                      {open ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
                    </button>

                    {open ? (
                      instrument.holdings.length === 0 ? (
                        <div className="mt-2 text-[0.66rem] text-zinc-600">Nothing held in any account.</div>
                      ) : (
                        <div className="mt-2 space-y-1.5">
                          {instrument.holdings.map((holding) => (
                            <div
                              key={`${holding.accountId}-${holding.instrumentId}`}
                              className="rounded-lg border border-white/[0.05] bg-white/[0.02] px-2.5 py-2"
                            >
                              <div className="flex items-center justify-between gap-2 text-[0.68rem]">
                                <span className="flex items-center gap-1.5 text-zinc-300">
                                  <Wallet className="h-3 w-3 text-zinc-600" /> {holding.accountName}
                                </span>
                                <span className="font-mono text-zinc-300">{holding.quantity}</span>
                              </div>
                              <div className="mt-1 grid grid-cols-3 gap-2 text-[0.6rem] text-zinc-600">
                                <span>Avg {holding.averageCost ?? "—"}</span>
                                <span>Cost {holding.costBasis}</span>
                                <span className={toneForSign(holding.unrealizedMinor)}>
                                  {holding.marketValue ?? "No valuation"}
                                  {holding.unrealized ? ` (${holding.unrealized})` : ""}
                                </span>
                              </div>
                            </div>
                          ))}
                        </div>
                      )
                    ) : null}
                  </div>

                  <div className="mt-auto flex flex-wrap items-center gap-1.5 pt-4">
                    <Button size="sm" variant="secondary" onClick={() => openCreateTrade(instrument)}>
                      <Plus className="h-3 w-3" /> Trade
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => openCreateValuation(instrument)}>
                      <Coins className="h-3 w-3" /> Valuation
                    </Button>
                  </div>
                </Panel>
              );
            })}
          </div>
        )
      ) : tab === "trades" ? (
        tradesQuery.isError ? (
          <Alert tone="error" title="Could not load trades">
            {errorMessage(tradesQuery.error)}
          </Alert>
        ) : tradesQuery.isLoading ? (
          <div className="flex items-center justify-center py-8 text-zinc-500">
            <Spinner className="h-4 w-4" />
          </div>
        ) : trades.length === 0 ? (
          <EmptyState
            icon={<TrendingUp className="h-5 w-5" />}
            title="No trades recorded"
            description="Record a buy or sell with an exact quantity and price. Nothing is inferred from account balances."
            action={
              <Button size="sm" variant="primary" onClick={() => openCreateTrade()}>
                <Plus className="h-3.5 w-3.5" /> Record first trade
              </Button>
            }
          />
        ) : (
          <Panel className="overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full min-w-[760px] text-left text-[0.72rem]">
                <thead className="border-b border-white/[0.06] text-[0.58rem] uppercase tracking-[0.14em] text-zinc-600">
                  <tr>
                    <th scope="col" className="px-3 py-2.5 font-medium">Date</th>
                    <th scope="col" className="px-3 py-2.5 font-medium">Instrument</th>
                    <th scope="col" className="px-3 py-2.5 font-medium">Side</th>
                    <th scope="col" className="px-3 py-2.5 font-medium">Account</th>
                    <th scope="col" className="px-3 py-2.5 text-right font-medium">Quantity</th>
                    <th scope="col" className="px-3 py-2.5 text-right font-medium">Price</th>
                    <th scope="col" className="px-3 py-2.5 text-right font-medium">Total</th>
                    <th scope="col" className="px-3 py-2.5 text-right font-medium">Fee</th>
                    <th scope="col" className="px-3 py-2.5" />
                  </tr>
                </thead>
                <tbody>
                  {trades.map((trade) => (
                    <tr key={trade.id} className="border-b border-white/[0.035] last:border-0">
                      <td className="px-3 py-2 text-zinc-400">{formatDate(trade.tradeDate)}</td>
                      <td className="px-3 py-2">
                        <div className="text-zinc-200">{trade.symbol}</div>
                        <div className="text-[0.6rem] text-zinc-600">{trade.instrumentName}</div>
                      </td>
                      <td className="px-3 py-2">
                        <Badge tone={trade.side === "buy" ? "green" : "red"}>
                          {trade.side === "buy" ? (
                            <ArrowDownRight className="h-3 w-3" />
                          ) : (
                            <ArrowUpRight className="h-3 w-3" />
                          )}
                          {trade.side}
                        </Badge>
                      </td>
                      <td className="px-3 py-2 text-zinc-400">{trade.accountName}</td>
                      <td className="px-3 py-2 text-right font-mono text-zinc-300">{trade.quantity}</td>
                      <td className="px-3 py-2 text-right font-mono text-zinc-300">{trade.price}</td>
                      <td className="px-3 py-2 text-right font-mono text-zinc-200">{trade.total}</td>
                      <td className="px-3 py-2 text-right font-mono text-zinc-500">{trade.fee}</td>
                      <td className="px-3 py-2">
                        <div className="flex justify-end gap-0.5">
                          <button
                            type="button"
                            className="rounded-md p-1 text-zinc-600 transition hover:text-zinc-200"
                            onClick={() => openEditTrade(trade)}
                            aria-label="Edit trade"
                          >
                            <Pencil className="h-3 w-3" />
                          </button>
                          <button
                            type="button"
                            className="rounded-md p-1 text-zinc-600 transition hover:text-lily-300"
                            onClick={() => {
                              if (window.confirm(`Delete the ${trade.side} of ${trade.quantity} ${trade.symbol}?`)) {
                                deleteTradeMutation.mutate(trade.id);
                              }
                            }}
                            aria-label="Delete trade"
                          >
                            <Trash2 className="h-3 w-3" />
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Panel>
        )
      ) : valuationsQuery.isError ? (
        <Alert tone="error" title="Could not load valuations">
          {errorMessage(valuationsQuery.error)}
        </Alert>
      ) : valuationsQuery.isLoading ? (
        <div className="flex items-center justify-center py-8 text-zinc-500">
          <Spinner className="h-4 w-4" />
        </div>
      ) : valuations.length === 0 ? (
        <EmptyState
          icon={<Coins className="h-5 w-5" />}
          title="No valuations recorded"
          description="Add a manual value from a broker statement. Rawjan will not fetch or invent a market price."
          action={
            <Button size="sm" variant="primary" onClick={() => openCreateValuation()}>
              <Plus className="h-3.5 w-3.5" /> Record first valuation
            </Button>
          }
        />
      ) : (
        <Panel className="overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full min-w-[680px] text-left text-[0.72rem]">
              <thead className="border-b border-white/[0.06] text-[0.58rem] uppercase tracking-[0.14em] text-zinc-600">
                <tr>
                  <th scope="col" className="px-3 py-2.5 font-medium">As of</th>
                  <th scope="col" className="px-3 py-2.5 font-medium">Instrument</th>
                  <th scope="col" className="px-3 py-2.5 font-medium">Account</th>
                  <th scope="col" className="px-3 py-2.5 text-right font-medium">Value</th>
                  <th scope="col" className="px-3 py-2.5 font-medium">Source</th>
                  <th scope="col" className="px-3 py-2.5" />
                </tr>
              </thead>
              <tbody>
                {valuations.map((valuation) => (
                  <tr key={valuation.id} className="border-b border-white/[0.035] last:border-0">
                    <td className="px-3 py-2 text-zinc-400">{formatDate(valuation.asOfDate)}</td>
                    <td className="px-3 py-2 text-zinc-200">{valuation.symbol ?? "Account-level"}</td>
                    <td className="px-3 py-2 text-zinc-400">{valuation.accountName}</td>
                    <td className="px-3 py-2 text-right font-mono text-zinc-200">{valuation.value}</td>
                    <td className="px-3 py-2 text-zinc-500">
                      {valuation.source === "manual" ? "Manual" : valuation.source}
                      {valuation.sourceRef ? ` · ${valuation.sourceRef}` : ""}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex justify-end gap-0.5">
                        <button
                          type="button"
                          className="rounded-md p-1 text-zinc-600 transition hover:text-zinc-200"
                          onClick={() => openEditValuation(valuation)}
                          aria-label="Edit valuation"
                        >
                          <Pencil className="h-3 w-3" />
                        </button>
                        <button
                          type="button"
                          className="rounded-md p-1 text-zinc-600 transition hover:text-lily-300"
                          onClick={() => {
                            if (window.confirm(`Delete the ${valuation.value} valuation from ${formatDate(valuation.asOfDate)}?`)) {
                              deleteValuationMutation.mutate(valuation.id);
                            }
                          }}
                          aria-label="Delete valuation"
                        >
                          <Trash2 className="h-3 w-3" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Panel>
      )}

      <Modal
        open={instrumentOpen}
        onClose={() => setInstrumentOpen(false)}
        title={editingInstrument ? "Edit instrument" : "New instrument"}
        description="Instruments are shared records. Trades and valuations reference them."
        width="lg"
        footer={
          <>
            <Button variant="ghost" onClick={() => setInstrumentOpen(false)} disabled={instrumentMutation.isPending}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submitInstrument} loading={instrumentMutation.isPending}>
              {editingInstrument ? "Save changes" : "Create instrument"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Symbol">
              <input
                className="rawjan-input font-mono uppercase"
                placeholder="e.g. VOO"
                value={instrumentForm.symbol}
                onChange={(event) => setInstrumentForm({ ...instrumentForm, symbol: event.target.value })}
                autoFocus
              />
            </Field>
            <Field label="Name">
              <input
                className="rawjan-input"
                placeholder="e.g. Vanguard S&P 500 ETF"
                value={instrumentForm.name}
                onChange={(event) => setInstrumentForm({ ...instrumentForm, name: event.target.value })}
              />
            </Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="Asset class">
              <select
                className="rawjan-input"
                value={instrumentForm.assetClass}
                onChange={(event) => setInstrumentForm({ ...instrumentForm, assetClass: event.target.value })}
              >
                {ASSET_CLASSES.map((item) => (
                  <option key={item.value} value={item.value}>
                    {item.label}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Currency" hint="Locked once trades exist.">
              <select
                className="rawjan-input"
                value={instrumentForm.currencyCode}
                onChange={(event) => setInstrumentForm({ ...instrumentForm, currencyCode: event.target.value })}
              >
                {currencies.map((currency) => (
                  <option key={currency.code} value={currency.code}>
                    {currency.code} — {currency.name}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Exchange" hint="Optional.">
              <input
                className="rawjan-input"
                placeholder="e.g. QSE"
                value={instrumentForm.exchange}
                onChange={(event) => setInstrumentForm({ ...instrumentForm, exchange: event.target.value })}
              />
            </Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="ISIN" hint="Optional identifier.">
              <input
                className="rawjan-input font-mono"
                placeholder="e.g. US9229087690"
                value={instrumentForm.isin}
                onChange={(event) => setInstrumentForm({ ...instrumentForm, isin: event.target.value })}
              />
            </Field>
            <Field label="Notes" hint="Optional.">
              <input
                className="rawjan-input"
                value={instrumentForm.notes}
                onChange={(event) => setInstrumentForm({ ...instrumentForm, notes: event.target.value })}
              />
            </Field>
          </div>
          {instrumentError ? (
            <Alert tone="error" title="Instrument not saved">
              {instrumentError}
            </Alert>
          ) : null}
        </div>
      </Modal>

      <Modal
        open={tradeOpen}
        onClose={() => setTradeOpen(false)}
        title={editingTrade ? "Edit trade" : "Record trade"}
        description="Trades are exact records: quantity and price are stored at eight decimal places. No price is inferred."
        width="lg"
        footer={
          <>
            <Button variant="ghost" onClick={() => setTradeOpen(false)} disabled={tradeMutation.isPending}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submitTrade} loading={tradeMutation.isPending}>
              {editingTrade ? "Save changes" : "Record trade"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Instrument">
              <select
                className="rawjan-input"
                value={tradeForm.instrumentId}
                onChange={(event) => setTradeForm({ ...tradeForm, instrumentId: event.target.value })}
              >
                {instruments.map((instrument) => (
                  <option key={instrument.id} value={instrument.id}>
                    {instrument.symbol} — {instrument.name}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Account">
              <select
                className="rawjan-input"
                value={tradeForm.accountId}
                onChange={(event) => setTradeForm({ ...tradeForm, accountId: event.target.value })}
              >
                {investmentAccounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name} · {account.currency}
                  </option>
                ))}
              </select>
            </Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-3">
            <Field label="Side">
              <select
                className="rawjan-input"
                value={tradeForm.side}
                onChange={(event) => setTradeForm({ ...tradeForm, side: event.target.value })}
              >
                <option value="buy">Buy</option>
                <option value="sell">Sell</option>
              </select>
            </Field>
            <Field label="Trade date">
              <input
                type="date"
                className="rawjan-input"
                value={tradeForm.tradeDate}
                onChange={(event) => setTradeForm({ ...tradeForm, tradeDate: event.target.value })}
              />
            </Field>
            <Field label="Fee" hint="Optional, same currency.">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={tradeForm.fee}
                onChange={(event) => setTradeForm({ ...tradeForm, fee: event.target.value })}
              />
            </Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Quantity">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0"
                value={tradeForm.quantity}
                onChange={(event) => setTradeForm({ ...tradeForm, quantity: event.target.value })}
              />
            </Field>
            <Field label={`Price (${tradeInstrument?.currency ?? "instrument currency"})`}>
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={tradeForm.price}
                onChange={(event) => setTradeForm({ ...tradeForm, price: event.target.value })}
              />
            </Field>
          </div>
          <Field label="Notes" hint="Optional.">
            <input
              className="rawjan-input"
              value={tradeForm.notes}
              onChange={(event) => setTradeForm({ ...tradeForm, notes: event.target.value })}
            />
          </Field>
          <Alert tone="info">
            A sell cannot exceed the quantity currently held in the selected account. Editing or deleting earlier buys is
            checked against later sells.
          </Alert>
          {tradeError ? (
            <Alert tone="error" title="Trade not saved">
              {tradeError}
            </Alert>
          ) : null}
        </div>
      </Modal>

      <Modal
        open={valuationOpen}
        onClose={() => setValuationOpen(false)}
        title={editingValuation ? "Edit valuation" : "Record manual valuation"}
        description="Use the total value shown by a statement. This is stored as a dated manual snapshot, never refreshed automatically."
        footer={
          <>
            <Button variant="ghost" onClick={() => setValuationOpen(false)} disabled={valuationMutation.isPending}>
              Cancel
            </Button>
            <Button variant="primary" onClick={submitValuation} loading={valuationMutation.isPending}>
              {editingValuation ? "Save changes" : "Record valuation"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <Field
              label="Instrument"
              hint="Choose “Account-level” for a value that covers the whole account."
            >
              <select
                className="rawjan-input"
                value={valuationForm.instrumentId}
                onChange={(event) => setValuationForm({ ...valuationForm, instrumentId: event.target.value })}
              >
                <option value="">Account-level (whole account)</option>
                {instruments.map((instrument) => (
                  <option key={instrument.id} value={instrument.id}>
                    {instrument.symbol} — {instrument.name}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Account">
              <select
                className="rawjan-input"
                value={valuationForm.accountId}
                onChange={(event) => setValuationForm({ ...valuationForm, accountId: event.target.value })}
              >
                {investmentAccounts.map((account) => (
                  <option key={account.id} value={account.id}>
                    {account.name} · {account.currency}
                  </option>
                ))}
              </select>
            </Field>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="As of date">
              <input
                type="date"
                className="rawjan-input"
                value={valuationForm.asOfDate}
                onChange={(event) => setValuationForm({ ...valuationForm, asOfDate: event.target.value })}
              />
            </Field>
            <Field label="Value" hint="Total value in the instrument's currency.">
              <input
                className="rawjan-input font-mono"
                inputMode="decimal"
                placeholder="0.00"
                value={valuationForm.value}
                onChange={(event) => setValuationForm({ ...valuationForm, value: event.target.value })}
              />
            </Field>
          </div>
          <Field label="Source reference" hint="Optional, e.g. “Q3 broker statement”.">
            <input
              className="rawjan-input"
              value={valuationForm.sourceRef}
              onChange={(event) => setValuationForm({ ...valuationForm, sourceRef: event.target.value })}
            />
          </Field>
          {valuationError ? (
            <Alert tone="error" title="Valuation not saved">
              {valuationError}
            </Alert>
          ) : null}
        </div>
      </Modal>
    </div>
  );
}

function Metric({
  label,
  value,
  tone = "text-zinc-200",
  mono = false,
}: {
  label: string;
  value: string;
  tone?: string;
  mono?: boolean;
}) {
  return (
    <div>
      <div className="text-[0.56rem] uppercase tracking-[0.14em] text-zinc-600">{label}</div>
      <div className={clsx("mt-0.5 text-[0.78rem]", mono && "font-mono", tone)}>{value}</div>
    </div>
  );
}
