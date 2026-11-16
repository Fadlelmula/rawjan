import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import {
  ArrowLeftRight,
  CircleAlert,
  LockKeyhole,
  RotateCcw,
  Trash2,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import { formatDate, formatDateTime, KIND_LABELS, signedAmount, STATUS_LABELS, titleCase } from "@/lib/display";
import { useToast } from "@/components/Toast";
import { Alert, Badge, Button, Modal, Spinner } from "@/components/ui";

export function statusTone(status: string): "default" | "green" | "amber" | "red" | "muted" | "champagne" {
  switch (status) {
    case "posted":
      return "green";
    case "reconciled":
      return "champagne";
    case "pending":
      return "amber";
    case "draft":
      return "muted";
    case "reversed":
      return "red";
    default:
      return "muted";
  }
}

export function TransactionDetailModal({
  id,
  onClose,
}: {
  id: string | null;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const toast = useToast();

  const detail = useQuery({
    queryKey: ["transaction", id],
    queryFn: () => api.getTransaction(id as string),
    enabled: Boolean(id),
  });

  const invalidateAll = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["transactions"] }),
      queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
      queryClient.invalidateQueries({ queryKey: ["transaction", id] }),
    ]);
  };

  const statusMutation = useMutation({
    mutationFn: (status: string) => api.setTransactionStatus(id as string, status),
    onSuccess: async () => {
      await invalidateAll();
      toast.success("Status updated");
    },
    onError: (error) => toast.error("Status not changed", errorMessage(error)),
  });

  const reverseMutation = useMutation({
    mutationFn: () => api.reverseTransaction(id as string),
    onSuccess: async () => {
      await invalidateAll();
      toast.success("Transaction reversed", "The original and its reversal remain in the ledger.");
      onClose();
    },
    onError: (error) => toast.error("Reversal failed", errorMessage(error)),
  });

  const deleteMutation = useMutation({
    mutationFn: () => api.deleteTransaction(id as string),
    onSuccess: async () => {
      await invalidateAll();
      toast.success("Draft deleted");
      onClose();
    },
    onError: (error) => toast.error("Draft not deleted", errorMessage(error)),
  });

  const transaction = detail.data;
  const canReverse = transaction?.status === "posted" || transaction?.status === "reconciled";
  const canPost = transaction?.status === "draft" || transaction?.status === "pending";
  const canReconcile = transaction?.status === "posted";
  const canDelete = transaction?.status === "draft";

  return (
    <Modal
      open={Boolean(id)}
      onClose={onClose}
      title={transaction?.description ?? "Transaction"}
      description={
        transaction
          ? `${formatDate(transaction.localDate)} · ${KIND_LABELS[transaction.kind] ?? transaction.kind}`
          : undefined
      }
      width="lg"
      footer={
        transaction ? (
          <>
            {canDelete ? (
              <Button
                variant="danger"
                onClick={() => {
                  if (window.confirm("Delete this draft? Drafts are the only records that can be removed.")) {
                    deleteMutation.mutate();
                  }
                }}
                loading={deleteMutation.isPending}
              >
                <Trash2 className="h-3.5 w-3.5" /> Delete draft
              </Button>
            ) : null}
            {canPost ? (
              <Button
                variant="secondary"
                onClick={() => statusMutation.mutate("posted")}
                loading={statusMutation.isPending}
              >
                Post to ledger
              </Button>
            ) : null}
            {canReconcile ? (
              <Button
                variant="secondary"
                onClick={() => statusMutation.mutate("reconciled")}
                loading={statusMutation.isPending}
              >
                Mark reconciled
              </Button>
            ) : null}
            {canReverse ? (
              <Button
                variant="danger"
                onClick={() => {
                  if (
                    window.confirm(
                      "Reverse this transaction? A mirrored reversal will be posted. The original is preserved and nothing is deleted.",
                    )
                  ) {
                    reverseMutation.mutate();
                  }
                }}
                loading={reverseMutation.isPending}
              >
                <RotateCcw className="h-3.5 w-3.5" /> Reverse
              </Button>
            ) : null}
            <Button variant="ghost" onClick={onClose}>
              Close
            </Button>
          </>
        ) : (
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        )
      }
    >
      {detail.isLoading ? (
        <div className="flex items-center justify-center py-14 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : detail.isError ? (
        <Alert tone="error" title="Could not load transaction">
          {errorMessage(detail.error)}
        </Alert>
      ) : transaction ? (
        <div className="space-y-5">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <div className="font-display text-3xl text-zinc-50">
                {signedAmount(transaction.kind, transaction.amount)}{" "}
                <span className="text-xs uppercase tracking-[0.16em] text-zinc-500">
                  {transaction.originalCurrency}
                </span>
              </div>
              {transaction.originalCurrency !== transaction.baseCurrency ? (
                <div className="mt-1 text-[0.68rem] text-zinc-500">
                  Base: {transaction.base} {transaction.baseCurrency}
                  {transaction.fxRate ? ` · rate ${transaction.fxRate}` : ""}
                </div>
              ) : null}
              <div className="mt-2 flex flex-wrap gap-1.5">
                <Badge tone={statusTone(transaction.status)}>{STATUS_LABELS[transaction.status] ?? transaction.status}</Badge>
                <Badge tone="default">{KIND_LABELS[transaction.kind] ?? titleCase(transaction.kind)}</Badge>
                <Badge tone={transaction.dataStatus === "actual" ? "muted" : "amber"}>
                  {titleCase(transaction.dataStatus)}
                </Badge>
                {transaction.reversalOfId ? <Badge tone="red">Reversal</Badge> : null}
              </div>
            </div>
            <div className="text-right text-[0.7rem] text-zinc-500">
              <div>{transaction.accountName}</div>
              {transaction.counterAccountName ? (
                <>
                  <ArrowLeftRight className="my-1 ml-auto h-3 w-3 text-zinc-600" />
                  <div>{transaction.counterAccountName}</div>
                </>
              ) : null}
            </div>
          </div>

          {transaction.status === "posted" || transaction.status === "reconciled" ? (
            <div className="flex items-start gap-2 rounded-xl border border-white/[0.06] bg-white/[0.02] px-3.5 py-2.5 text-[0.66rem] leading-relaxed text-zinc-500">
              <LockKeyhole className="mt-0.5 h-3.5 w-3.5 shrink-0 text-champagne-500" />
              Posted amounts and ledger legs are immutable. Use a reversal to correct this record; the history
              remains auditable.
            </div>
          ) : null}

          <div className="grid gap-2 text-[0.72rem] sm:grid-cols-2">
            <DetailRow label="Payee" value={transaction.payee ?? "—"} />
            <DetailRow label="Category" value={transaction.categoryName ?? (transaction.isSplit ? "Split" : "—")} />
            <DetailRow label="Local date" value={formatDate(transaction.localDate)} />
            <DetailRow label="Occurred at" value={formatDateTime(transaction.occurredAt)} />
            <DetailRow label="Created" value={transaction.createdAt.replace("T", " ").replace("Z", " UTC")} />
            <DetailRow label="Posted" value={transaction.postedAt?.replace("T", " ").replace("Z", " UTC") ?? "—"} />
            <DetailRow label="External reference" value={transaction.externalRef ?? "—"} />
            <DetailRow
              label="FX rate"
              value={transaction.fxRate ? `${transaction.fxRate} (${transaction.fxRateDate ?? "snapshot"})` : "—"}
            />
          </div>

          {transaction.notes ? (
            <div className="rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-3 text-[0.72rem] leading-relaxed text-zinc-400">
              {transaction.notes}
            </div>
          ) : null}

          {transaction.tags.length > 0 ? (
            <div className="flex flex-wrap gap-1.5">
              {transaction.tags.map((tag) => (
                <Badge key={tag.id} tone="muted">
                  #{tag.name}
                </Badge>
              ))}
            </div>
          ) : null}

          {transaction.splits.length > 0 ? (
            <section>
              <div className="mb-2 text-[0.58rem] font-semibold uppercase tracking-[0.18em] text-zinc-600">
                Splits
              </div>
              <div className="overflow-hidden rounded-xl border border-white/[0.06]">
                {transaction.splits.map((split, index) => (
                  <div
                    key={split.id}
                    className={clsx(
                      "flex items-center justify-between gap-3 px-3.5 py-2 text-[0.72rem]",
                      index > 0 && "border-t border-white/[0.05]",
                    )}
                  >
                    <div className="min-w-0">
                      <div className="truncate text-zinc-300">{split.categoryName}</div>
                      {split.note ? <div className="truncate text-[0.62rem] text-zinc-600">{split.note}</div> : null}
                    </div>
                    <div className="font-mono text-zinc-400">
                      {split.amount} {transaction.originalCurrency}
                    </div>
                  </div>
                ))}
              </div>
            </section>
          ) : null}

          <section>
            <div className="mb-2 text-[0.58rem] font-semibold uppercase tracking-[0.18em] text-zinc-600">
              Double-entry legs
            </div>
            <div className="overflow-hidden rounded-xl border border-white/[0.06]">
              {transaction.ledgerEntries.map((entry, index) => (
                <div
                  key={entry.id}
                  className={clsx(
                    "grid grid-cols-[64px_1fr_auto] items-center gap-3 px-3.5 py-2 text-[0.7rem]",
                    index > 0 && "border-t border-white/[0.05]",
                  )}
                >
                  <span
                    className={clsx(
                      "text-[0.58rem] font-semibold uppercase tracking-wider",
                      entry.entryType === "debit" ? "text-sage-400" : "text-lily-300",
                    )}
                  >
                    {entry.entryType}
                  </span>
                  <div className="min-w-0">
                    <div className="truncate text-zinc-300">
                      {entry.accountName ?? entry.categoryName ?? "—"}
                    </div>
                    <div className="text-[0.6rem] text-zinc-600">
                      {entry.accountName ? "Account" : "Category"}
                    </div>
                  </div>
                  <div className="text-right font-mono text-zinc-400">
                    {entry.amount} {entry.currency}
                    <div className="text-[0.58rem] text-zinc-600">
                      base {entry.baseAmount} {transaction.baseCurrency}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </section>

          {transaction.status === "reversed" ? (
            <Alert tone="warning" title="This record has been reversed">
              Its mirrored reversal is a separate posted transaction. Both records remain visible for audit.
            </Alert>
          ) : null}
          {transaction.status === "cancelled" ? (
            <Alert tone="error" title="Cancelled before posting">
              <span className="inline-flex items-center gap-1.5">
                <CircleAlert className="h-3.5 w-3.5" /> Cancelled records never affected balances.
              </span>
            </Alert>
          ) : null}
        </div>
      ) : null}
    </Modal>
  );
}

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start justify-between gap-3 rounded-lg border border-white/[0.05] bg-white/[0.015] px-3 py-2">
      <span className="text-[0.6rem] uppercase tracking-[0.13em] text-zinc-600">{label}</span>
      <span className="max-w-[62%] break-words text-right text-zinc-300">{value}</span>
    </div>
  );
}
