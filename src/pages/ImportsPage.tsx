import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CheckCircle2,
  FileSpreadsheet,
  FileUp,
  History,
  ShieldAlert,
  TriangleAlert,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type {
  ColumnMapping,
  ImportJob,
  ImportPreview,
  ImportResult,
} from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { formatDate, titleCase } from "@/lib/display";
import { useToast } from "@/components/Toast";
import {
  Alert,
  Badge,
  Button,
  EmptyState,
  Field,
  Panel,
  SectionLabel,
  Spinner,
} from "@/components/ui";

const FORMATS = [
  { value: "csv", label: "CSV — comma or semicolon separated" },
  { value: "xlsx", label: "XLSX — first worksheet" },
  { value: "ofx", label: "OFX — bank statement export" },
  { value: "qif", label: "QIF — Quicken interchange" },
];

const MAPPING_FIELDS: Array<{ key: MappingColumnKey; label: string }> = [
  { key: "date", label: "Date column" },
  { key: "description", label: "Description column" },
  { key: "amount", label: "Signed amount column" },
  { key: "debit", label: "Debit (money out) column" },
  { key: "credit", label: "Credit (money in) column" },
  { key: "payee", label: "Payee column" },
  { key: "category", label: "Category column" },
  { key: "currency", label: "Currency column" },
  { key: "reference", label: "Reference column" },
];

type MappingColumnKey =
  | "date"
  | "description"
  | "amount"
  | "debit"
  | "credit"
  | "payee"
  | "category"
  | "currency"
  | "reference";

type MappingOptionKey = "dateOrder" | "decimalSeparator" | "defaultKind" | "status";

function extensionOf(path: string): string {
  const match = /\.([a-z0-9]+)$/i.exec(path.trim());
  return match ? match[1].toLowerCase() : "";
}

function statusTone(status: string) {
  switch (status) {
    case "committed":
    case "imported":
      return "green" as const;
    case "parsed":
    case "ok":
      return "champagne" as const;
    case "cancelled":
    case "skipped":
      return "muted" as const;
    case "duplicate":
      return "amber" as const;
    default:
      return "red" as const;
  }
}

export function ImportsPage() {
  const { accounts } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();

  const [accountId, setAccountId] = useState("");
  const [format, setFormat] = useState("csv");
  const [filePath, setFilePath] = useState<string | null>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [mapping, setMapping] = useState<ColumnMapping | null>(null);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const activeAccounts = useMemo(
    () => accounts.filter((account) => account.isActive),
    [accounts],
  );

  const jobsQuery = useQuery({
    queryKey: ["imports"],
    queryFn: () => api.listImportJobs(),
  });

  const refreshAll = async () => {
    await queryClient.invalidateQueries();
  };

  const pickFile = async () => {
    setError(null);
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Bank statements", extensions: ["csv", "xlsx", "ofx", "qif"] }],
      });
      if (typeof selected !== "string") return;
      setFilePath(selected);
      setPreview(null);
      setMapping(null);
      setResult(null);
      const extension = extensionOf(selected);
      if (["csv", "xlsx", "ofx", "qif"].includes(extension)) {
        setFormat(extension);
      }
    } catch (cause) {
      setError(errorMessage(cause));
    }
  };

  const previewMutation = useMutation({
    mutationFn: () => {
      if (!filePath) throw new Error("Choose a statement file first.");
      if (!accountId) throw new Error("Choose the account this statement belongs to.");
      return api.importPreview(filePath, format, accountId);
    },
    onSuccess: (data) => {
      setPreview(data);
      setMapping(data.suggestedMapping);
      setResult(null);
      setError(null);
      if (data.warnings.length > 0) {
        toast.success("Statement read", data.warnings[0]);
      }
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  const commitMutation = useMutation({
    mutationFn: () => {
      if (!preview || !mapping) throw new Error("Preview the file before importing.");
      return api.importCommit(preview.jobId, mapping, preview.headers, accountId || null);
    },
    onSuccess: async (data) => {
      setResult(data);
      setError(null);
      await refreshAll();
      toast.success(
        data.imported > 0 ? "Statement imported" : "Nothing new was imported",
        `${data.imported} imported · ${data.duplicates} duplicates · ${data.errors} errors · ${data.skipped} skipped`,
      );
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  const cancelMutation = useMutation({
    mutationFn: (jobId: string) => api.cancelImport(jobId),
    onSuccess: async () => {
      setPreview(null);
      setMapping(null);
      setResult(null);
      setError(null);
      await queryClient.invalidateQueries({ queryKey: ["imports"] });
      toast.success("Preview discarded", "No transactions were written.");
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  const updateColumn = (key: MappingColumnKey, value: string) => {
    setMapping((current) => (current ? { ...current, [key]: value || null } : current));
  };

  const updateOption = (key: MappingOptionKey, value: string) => {
    setMapping((current) => (current ? { ...current, [key]: value } : current));
  };

  const setSkipDuplicates = (value: boolean) => {
    setMapping((current) => (current ? { ...current, skipDuplicates: value } : current));
  };

  const tabular = preview ? ["csv", "xlsx"].includes(preview.format) : false;
  const jobs = jobsQuery.data ?? [];

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Bring in real records
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">
            Statement import
          </h2>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
            Read a statement you exported from your bank, check every row, then commit. Imports
            never contact a bank, never ask for a password, and nothing is written until you press
            import.
          </p>
        </div>
        <Badge tone="champagne">CSV · XLSX · OFX · QIF</Badge>
      </div>

      {error ? (
        <Alert tone="error" title="Import stopped">
          {error}
        </Alert>
      ) : null}

      <Panel className="space-y-4 p-5">
        <div className="flex items-center gap-2">
          <FileUp className="h-4 w-4 text-lily-300" />
          <SectionLabel>1 · Choose the file and account</SectionLabel>
        </div>
        <div className="grid gap-3 md:grid-cols-3">
          <Field label="Account" hint="The statement must belong to one account.">
            <select
              className="rawjan-input"
              value={accountId}
              onChange={(event) => {
                setAccountId(event.target.value);
                setPreview(null);
                setMapping(null);
                setResult(null);
              }}
            >
              <option value="">Choose an account…</option>
              {activeAccounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.name} · {account.currency}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Format" hint="Detected from the extension; you can change it.">
            <select
              className="rawjan-input"
              value={format}
              onChange={(event) => {
                setFormat(event.target.value);
                setPreview(null);
                setMapping(null);
                setResult(null);
              }}
            >
              {FORMATS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Statement file" hint="Up to 64 MB. Nothing is uploaded.">
            <div className="flex gap-2">
              <Button size="sm" variant="secondary" className="h-10 flex-1" onClick={pickFile}>
                <FileSpreadsheet className="h-3.5 w-3.5" /> Choose file
              </Button>
            </div>
          </Field>
        </div>

        <div className="flex flex-wrap items-center gap-3 rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-2.5">
          <div className="min-w-0 flex-1 truncate font-mono text-[0.68rem] text-zinc-500">
            {filePath ?? "No file chosen yet."}
          </div>
          <Button
            size="sm"
            variant="primary"
            loading={previewMutation.isPending}
            disabled={!filePath || !accountId}
            onClick={() => previewMutation.mutate()}
          >
            Read and preview
          </Button>
        </div>

        <div className="text-[0.62rem] leading-relaxed text-zinc-600">
          PDF statements are not parsed in this build: layouts vary too much to read reliably, so
          Rawjan refuses rather than guessing. Export CSV, OFX or QIF from your bank instead.
        </div>
      </Panel>

      {preview ? (
        <Panel className="space-y-4 p-5">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <CheckCircle2 className="h-4 w-4 text-sage-400" />
              <SectionLabel>2 · Check the rows, then map the columns</SectionLabel>
            </div>
            <div className="flex flex-wrap items-center gap-1.5">
              <Badge tone="muted">{preview.fileName}</Badge>
              <Badge tone="default">{preview.rowCount} rows</Badge>
              <Badge tone="champagne">{preview.format.toUpperCase()}</Badge>
            </div>
          </div>

          {preview.warnings.length > 0 ? (
            <Alert tone="warning" title="Read this before importing">
              <ul className="list-inside list-disc space-y-1">
                {preview.warnings.map((warning) => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            </Alert>
          ) : null}

          {tabular ? (
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {MAPPING_FIELDS.map((field) => (
                <Field key={field.key} label={field.label}>
                  <select
                    className="rawjan-input"
                    value={(mapping?.[field.key] as string | null) ?? ""}
                    onChange={(event) => updateColumn(field.key, event.target.value)}
                  >
                    <option value="">Not mapped</option>
                    {preview.headers.map((header) => (
                      <option key={header} value={header}>
                        {header}
                      </option>
                    ))}
                  </select>
                </Field>
              ))}
            </div>
          ) : (
            <div className="rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-2.5 text-[0.68rem] leading-relaxed text-zinc-500">
              {preview.format.toUpperCase()} rows are mapped automatically. Transfers and
              unsupported records are skipped with a reason, never guessed.
            </div>
          )}

          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-5">
            <Field label="Date order">
              <select
                className="rawjan-input"
                value={mapping?.dateOrder ?? "auto"}
                onChange={(event) => updateOption("dateOrder", event.target.value)}
              >
                <option value="auto">Auto (day-first when ambiguous)</option>
                <option value="dmy">Day first · 31/12/2026</option>
                <option value="mdy">Month first · 12/31/2026</option>
                <option value="ymd">Year first · 2026-12-31</option>
              </select>
            </Field>
            <Field label="Decimal separator">
              <select
                className="rawjan-input"
                value={mapping?.decimalSeparator ?? "auto"}
                onChange={(event) => updateOption("decimalSeparator", event.target.value)}
              >
                <option value="auto">Auto detect</option>
                <option value="dot">Dot · 1,234.56</option>
                <option value="comma">Comma · 1.234,56</option>
              </select>
            </Field>
            <Field label="Default type" hint="Used when the sign is ambiguous.">
              <select
                className="rawjan-input"
                value={mapping?.defaultKind ?? "auto"}
                onChange={(event) => updateOption("defaultKind", event.target.value)}
              >
                <option value="auto">Auto from sign</option>
                <option value="expense">Always expense</option>
                <option value="income">Always income</option>
              </select>
            </Field>
            <Field label="Row status">
              <select
                className="rawjan-input"
                value={mapping?.status ?? "posted"}
                onChange={(event) => updateOption("status", event.target.value)}
              >
                <option value="posted">Posted</option>
                <option value="pending">Pending</option>
              </select>
            </Field>
            <label className="flex items-end gap-2 pb-2 text-[0.68rem] text-zinc-400">
              <input
                type="checkbox"
                className="h-3.5 w-3.5 accent-lily-500"
                checked={mapping?.skipDuplicates ?? true}
                onChange={(event) => setSkipDuplicates(event.target.checked)}
              />
              Skip duplicates
            </label>
          </div>

          <div className="text-[0.62rem] leading-relaxed text-zinc-600">
            The table shows what was detected with the suggested mapping. Changing a column changes
            what will be committed; the row values below are not re-parsed on screen.
          </div>

          <div className="max-h-[26rem] overflow-auto rounded-xl border border-white/[0.06]">
            <table className="w-full min-w-[52rem] border-collapse text-left text-[0.68rem]">
              <thead className="sticky top-0 z-10 bg-[#0c0b0c]">
                <tr className="border-b border-white/[0.08] text-[0.6rem] uppercase tracking-[0.12em] text-zinc-500">
                  <th className="px-2.5 py-2 font-medium">#</th>
                  {preview.headers.map((header) => (
                    <th key={header} className="px-2.5 py-2 font-medium">
                      {header}
                    </th>
                  ))}
                  <th className="px-2.5 py-2 font-medium">Detected</th>
                  <th className="px-2.5 py-2 font-medium">State</th>
                </tr>
              </thead>
              <tbody>
                {preview.rows.map((row) => {
                  const normalized = (row.normalized ?? {}) as Record<string, unknown>;
                  const detected = [
                    typeof normalized.date === "string" ? normalized.date : null,
                    typeof normalized.amount === "string" ? normalized.amount : null,
                    typeof normalized.description === "string" ? normalized.description : null,
                  ]
                    .filter(Boolean)
                    .join(" · ");
                  return (
                    <tr
                      key={row.rowIndex}
                      className="border-b border-white/[0.04] align-top last:border-b-0"
                    >
                      <td className="px-2.5 py-1.5 font-mono text-zinc-600">{row.rowIndex + 1}</td>
                      {preview.headers.map((header, index) => (
                        <td
                          key={`${row.rowIndex}-${header}`}
                          className="max-w-[16rem] truncate px-2.5 py-1.5 text-zinc-400"
                          title={row.cells[index] ?? ""}
                        >
                          {row.cells[index] ?? ""}
                        </td>
                      ))}
                      <td className="max-w-[18rem] px-2.5 py-1.5 text-zinc-300">
                        {detected || <span className="text-zinc-600">Nothing detected</span>}
                      </td>
                      <td className="px-2.5 py-1.5">
                        <Badge tone={statusTone(row.status)}>{titleCase(row.status)}</Badge>
                        {row.issue ? (
                          <div className="mt-1 max-w-[16rem] text-[0.6rem] leading-snug text-lily-300/90">
                            {row.issue}
                          </div>
                        ) : null}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {preview.rowCount > preview.rows.length ? (
            <div className="text-[0.62rem] text-zinc-600">
              Showing the first {preview.rows.length} of {preview.rowCount} rows. Every row is
              validated when you import.
            </div>
          ) : null}

          <div className="flex flex-wrap items-center gap-2 border-t border-white/[0.06] pt-3">
            <Button
              size="sm"
              variant="primary"
              loading={commitMutation.isPending}
              onClick={() => commitMutation.mutate()}
            >
              Import {preview.rowCount} rows
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={cancelMutation.isPending}
              onClick={() => cancelMutation.mutate(preview.jobId)}
            >
              Discard preview
            </Button>
            <div className="ml-auto text-[0.62rem] text-zinc-600">
              Fingerprint {preview.fileHash.slice(0, 12)}…
            </div>
          </div>
        </Panel>
      ) : null}

      {result ? (
        <Panel className="space-y-3 p-5">
          <div className="flex items-center gap-2">
            <CheckCircle2 className="h-4 w-4 text-sage-400" />
            <SectionLabel>3 · Import result</SectionLabel>
          </div>
          <div className="flex flex-wrap gap-2">
            <Badge tone="green">{result.imported} imported</Badge>
            <Badge tone="amber">{result.duplicates} duplicates</Badge>
            <Badge tone={result.errors > 0 ? "red" : "muted"}>{result.errors} errors</Badge>
            <Badge tone="muted">{result.skipped} skipped</Badge>
          </div>
          {result.issues.length > 0 ? (
            <Alert tone="warning" title="Rows that could not be imported">
              <ul className="space-y-1">
                {result.issues.map((issue) => (
                  <li key={`${issue.rowIndex}-${issue.message}`}>
                    Row {issue.rowIndex + 1}: {issue.message}
                  </li>
                ))}
              </ul>
            </Alert>
          ) : (
            <div className="text-[0.68rem] text-zinc-500">
              Every row that could be read became a real ledger transaction. Nothing was invented.
            </div>
          )}
          <Button
            size="sm"
            variant="secondary"
            onClick={() => {
              setPreview(null);
              setMapping(null);
              setResult(null);
              setFilePath(null);
            }}
          >
            Import another file
          </Button>
        </Panel>
      ) : null}

      <Panel className="space-y-3 p-5">
        <div className="flex items-center gap-2">
          <History className="h-4 w-4 text-zinc-500" />
          <SectionLabel>Import history</SectionLabel>
        </div>
        {jobsQuery.isLoading ? (
          <div className="flex items-center justify-center py-8 text-zinc-500">
            <Spinner className="h-4 w-4" />
          </div>
        ) : jobs.length === 0 ? (
          <EmptyState
            icon={<FileUp className="h-5 w-5" />}
            title="No imports yet"
            description="Every preview you start is recorded here with its file fingerprint, so a statement is never silently imported twice."
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[44rem] border-collapse text-left text-[0.68rem]">
              <thead>
                <tr className="border-b border-white/[0.08] text-[0.6rem] uppercase tracking-[0.12em] text-zinc-500">
                  <th className="py-2 pr-3 font-medium">File</th>
                  <th className="py-2 pr-3 font-medium">Format</th>
                  <th className="py-2 pr-3 font-medium">Status</th>
                  <th className="py-2 pr-3 text-right font-medium">Rows</th>
                  <th className="py-2 pr-3 text-right font-medium">Imported</th>
                  <th className="py-2 pr-3 text-right font-medium">Dupes</th>
                  <th className="py-2 pr-3 text-right font-medium">Errors</th>
                  <th className="py-2 font-medium">Started</th>
                </tr>
              </thead>
              <tbody>
                {jobs.map((job: ImportJob) => (
                  <tr key={job.id} className="border-b border-white/[0.04] last:border-b-0">
                    <td className="max-w-[18rem] truncate py-2 pr-3 text-zinc-300" title={job.fileName}>
                      {job.fileName}
                    </td>
                    <td className="py-2 pr-3 uppercase text-zinc-500">{job.format}</td>
                    <td className="py-2 pr-3">
                      <Badge tone={statusTone(job.status)}>{titleCase(job.status)}</Badge>
                    </td>
                    <td className="py-2 pr-3 text-right font-mono text-zinc-400">{job.rowCount}</td>
                    <td className="py-2 pr-3 text-right font-mono text-sage-400">
                      {job.importedCount}
                    </td>
                    <td className="py-2 pr-3 text-right font-mono text-amber-300">
                      {job.duplicateCount}
                    </td>
                    <td className="py-2 pr-3 text-right font-mono text-lily-300">
                      {job.errorCount}
                    </td>
                    <td className="py-2 text-zinc-500">
                      {formatDate(job.createdAt.slice(0, 10))}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Panel>

      <div className="flex items-start gap-2 rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-2.5 text-[0.62rem] leading-relaxed text-zinc-600">
        <TriangleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0 text-zinc-600" />
        <span>
          Transfers, split categories and investment actions inside QIF/OFX files are skipped with
          an explicit reason. Record those manually so both sides of the movement stay balanced.
        </span>
      </div>

      <div className="flex items-start gap-2 rounded-xl border border-lily-500/20 bg-lily-600/[0.06] px-3.5 py-2.5 text-[0.62rem] leading-relaxed text-lily-200/80">
        <ShieldAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
        <span>
          Statement files are read on this device and nothing is sent to a bank or any other
          service. Manual entry is always available for anything the importer cannot read.
        </span>
      </div>
    </div>
  );
}
