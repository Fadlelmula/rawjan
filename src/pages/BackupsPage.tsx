import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Archive,
  CheckCircle2,
  Database,
  FileSpreadsheet,
  KeyRound,
  LockKeyhole,
  RefreshCw,
  ShieldCheck,
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { Backup, BackupValidation, ExportResult, ExportSummary } from "@/lib/api";
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

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function kindTone(kind: string) {
  switch (kind) {
    case "manual":
      return "champagne" as const;
    case "pre_migration":
      return "amber" as const;
    case "export":
      return "default" as const;
    default:
      return "muted" as const;
  }
}

export function BackupsPage() {
  const { app } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();

  const [error, setError] = useState<string | null>(null);
  const [validation, setValidation] = useState<BackupValidation | null>(null);
  const [passphrase, setPassphrase] = useState("");
  const [confirmPassphrase, setConfirmPassphrase] = useState("");
  const [exportResult, setExportResult] = useState<ExportResult | null>(null);
  const [verifyPath, setVerifyPath] = useState<string | null>(null);
  const [verifyPassphrase, setVerifyPassphrase] = useState("");
  const [verifySummary, setVerifySummary] = useState<ExportSummary | null>(null);

  const backupsQuery = useQuery({
    queryKey: ["backups"],
    queryFn: () => api.listBackups(),
  });

  const createMutation = useMutation({
    mutationFn: () => api.createBackup("manual"),
    onSuccess: async (backup) => {
      setError(null);
      await queryClient.invalidateQueries({ queryKey: ["backups"] });
      toast.success(
        "Backup created and verified",
        `${backup.fileName} · ${formatBytes(backup.byteSize)} · integrity check passed`,
      );
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.deleteBackup(id),
    onSuccess: async () => {
      setError(null);
      await queryClient.invalidateQueries({ queryKey: ["backups"] });
      toast.success("Backup deleted", "The file was removed from the managed backup folder.");
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  const validateMutation = useMutation({
    mutationFn: (path: string) => api.validateBackup(path),
    onSuccess: (data) => {
      setValidation(data);
      setError(null);
    },
    onError: (cause) => {
      setValidation(null);
      setError(errorMessage(cause));
    },
  });

  const exportMutation = useMutation({
    mutationFn: () => api.createEncryptedExport(passphrase),
    onSuccess: (data) => {
      setExportResult(data);
      setError(null);
      setPassphrase("");
      setConfirmPassphrase("");
      toast.success(
        "Encrypted export created",
        `${data.fileName} · ${formatBytes(data.byteSize)} · round-trip verified`,
      );
    },
    onError: (cause) => setError(errorMessage(cause)),
  });

  const verifyMutation = useMutation({
    mutationFn: () => {
      if (!verifyPath) throw new Error("Choose an export file first.");
      return api.verifyEncryptedExport(verifyPath, verifyPassphrase);
    },
    onSuccess: (data) => {
      setVerifySummary(data);
      setError(null);
      setVerifyPassphrase("");
    },
    onError: (cause) => {
      setVerifySummary(null);
      setError(errorMessage(cause));
    },
  });

  const pickExportFile = async () => {
    setError(null);
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Rawjan export", extensions: ["rawjan", "aureqis"] }],
      });
      if (typeof selected !== "string") return;
      setVerifyPath(selected);
      setVerifySummary(null);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  };

  const submitExport = () => {
    if (passphrase.trim().length < 8) {
      setError("Use a passphrase of at least 8 characters. It is never stored.");
      return;
    }
    if (passphrase !== confirmPassphrase) {
      setError("The two passphrases do not match.");
      return;
    }
    exportMutation.mutate();
  };

  const backups = backupsQuery.data ?? [];
  const backupFiles = backups.filter((backup) => backup.kind !== "export");
  const exports = backups.filter((backup) => backup.kind === "export");

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Nothing leaves this machine
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">
            Backups &amp; exports
          </h2>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
            Backups are verified copies of the encrypted database. Exports are encrypted JSON that
            can be read outside Rawjan. Restoring is deliberately manual: the live database is
            never swapped underneath a running app.
          </p>
        </div>
        <div className="flex flex-wrap gap-1.5">
          <Badge tone="green">
            <LockKeyhole className="h-3 w-3" /> Encrypted at rest
          </Badge>
          <Badge tone="muted">Data folder {app.dataDir}</Badge>
        </div>
      </div>

      {error ? (
        <Alert tone="error" title="Nothing was changed">
          {error}
        </Alert>
      ) : null}

      <Panel className="space-y-4 p-5">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Database className="h-4 w-4 text-lily-300" />
            <SectionLabel>Encrypted database backups</SectionLabel>
          </div>
          <Button
            size="sm"
            variant="primary"
            loading={createMutation.isPending}
            onClick={() => createMutation.mutate()}
          >
            <RefreshCw className="h-3.5 w-3.5" /> Create verified backup
          </Button>
        </div>

        <div className="text-[0.68rem] leading-relaxed text-zinc-500">
          A backup copies the SQLCipher database with its page-level encryption intact and checks
          its integrity immediately. The key stays in Windows Credential Manager, so a backup file
          is useless without this Windows account.
        </div>

        {backupsQuery.isLoading ? (
          <div className="flex items-center justify-center py-8 text-zinc-500">
            <Spinner className="h-4 w-4" />
          </div>
        ) : backupFiles.length === 0 ? (
          <EmptyState
            icon={<Database className="h-5 w-5" />}
            title="No database backup yet"
            description="Create one before changing anything important. The first backup is the start of an honest recovery path."
          />
        ) : (
          <div className="space-y-2">
            {backupFiles.map((backup: Backup) => (
              <div
                key={backup.id}
                className="flex flex-wrap items-center gap-3 rounded-xl border border-white/[0.06] bg-black/20 px-3.5 py-2.5"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="truncate font-mono text-[0.7rem] text-zinc-200">
                      {backup.fileName}
                    </span>
                    <Badge tone={kindTone(backup.kind)}>{titleCase(backup.kind)}</Badge>
                    {backup.verifiedAt ? (
                      <Badge tone="green">
                        <CheckCircle2 className="h-3 w-3" /> Verified
                      </Badge>
                    ) : (
                      <Badge tone="amber">Not verified</Badge>
                    )}
                  </div>
                  <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-[0.6rem] text-zinc-600">
                    <span>{formatBytes(backup.byteSize)}</span>
                    <span>Schema v{backup.schemaVersion}</span>
                    <span>{formatDate(backup.createdAt.slice(0, 10))}</span>
                    <span className="font-mono">sha256 {backup.sha256.slice(0, 16)}…</span>
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-1">
                  <Button
                    size="sm"
                    variant="secondary"
                    loading={validateMutation.isPending}
                    onClick={() => validateMutation.mutate(backup.path)}
                  >
                    <ShieldCheck className="h-3.5 w-3.5" /> Validate
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    loading={deleteMutation.isPending}
                    onClick={() => {
                      if (
                        window.confirm(
                          `Delete the backup “${backup.fileName}”? The file is removed from disk.`,
                        )
                      ) {
                        deleteMutation.mutate(backup.id);
                      }
                    }}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}

        {validation ? (
          <Alert
            tone={validation.readable ? "success" : "error"}
            title={validation.readable ? "Backup opens and passes integrity" : "Backup failed"}
          >
            <div className="space-y-1">
              <div>{validation.message}</div>
              <div className="text-[0.62rem] text-zinc-500">
                {validation.fileName} · integrity {validation.integrity}
                {validation.schemaVersion !== null
                  ? ` · schema v${validation.schemaVersion}`
                  : ""}
                {validation.encrypted ? " · encrypted" : ""}
              </div>
              {validation.tables.length > 0 ? (
                <div className="flex flex-wrap gap-1.5 pt-1">
                  {validation.tables
                    .filter((table) => table.rows > 0)
                    .map((table) => (
                      <Badge key={table.table} tone="muted">
                        {table.table} {table.rows}
                      </Badge>
                    ))}
                </div>
              ) : null}
            </div>
          </Alert>
        ) : null}
      </Panel>

      <Panel className="space-y-4 p-5">
        <div className="flex items-center gap-2">
          <FileSpreadsheet className="h-4 w-4 text-champagne-300" />
          <SectionLabel>Encrypted JSON export</SectionLabel>
        </div>
        <div className="text-[0.68rem] leading-relaxed text-zinc-500">
          An export contains your tenant rows as readable JSON, sealed with Argon2id and
          ChaCha20-Poly1305. The passphrase is never stored. Losing it means losing the export.
        </div>
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Passphrase" hint="At least 8 characters. Not saved anywhere.">
            <input
              type="password"
              className="rawjan-input"
              autoComplete="new-password"
              value={passphrase}
              onChange={(event) => setPassphrase(event.target.value)}
            />
          </Field>
          <Field label="Confirm passphrase">
            <input
              type="password"
              className="rawjan-input"
              autoComplete="new-password"
              value={confirmPassphrase}
              onChange={(event) => setConfirmPassphrase(event.target.value)}
            />
          </Field>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            variant="primary"
            loading={exportMutation.isPending}
            disabled={!passphrase || !confirmPassphrase}
            onClick={submitExport}
          >
            <Archive className="h-3.5 w-3.5" /> Create encrypted export
          </Button>
          <span className="text-[0.62rem] text-zinc-600">
            Files are written to {app.dataDir}\exports
          </span>
        </div>

        {exportResult ? (
          <Alert tone="success" title="Export written and verified">
            <div className="space-y-1">
              <div className="break-all font-mono text-[0.65rem]">{exportResult.path}</div>
              <div className="text-[0.62rem] text-zinc-500">
                {formatBytes(exportResult.byteSize)} · sha256 {exportResult.sha256.slice(0, 20)}…
              </div>
              <div className="flex flex-wrap gap-1.5 pt-1">
                {exportResult.tableCounts
                  .filter((table) => table.rows > 0)
                  .map((table) => (
                    <Badge key={table.table} tone="muted">
                      {table.table} {table.rows}
                    </Badge>
                  ))}
              </div>
            </div>
          </Alert>
        ) : null}

        <div className="border-t border-white/[0.06] pt-4">
          <div className="mb-2 flex items-center gap-2">
            <KeyRound className="h-3.5 w-3.5 text-zinc-500" />
            <SectionLabel>Verify an export</SectionLabel>
          </div>
          <div className="grid gap-3 sm:grid-cols-[1.4fr_1fr_auto] sm:items-end">
            <Field label="Export file">
              <div className="flex gap-2">
                <div className="rawjan-input flex min-w-0 flex-1 items-center truncate font-mono text-[0.65rem] text-zinc-500">
                  {verifyPath ?? "No file chosen"}
                </div>
                <Button size="sm" variant="secondary" className="h-10" onClick={pickExportFile}>
                  Choose
                </Button>
              </div>
            </Field>
            <Field label="Passphrase">
              <input
                type="password"
                className="rawjan-input"
                autoComplete="off"
                value={verifyPassphrase}
                onChange={(event) => setVerifyPassphrase(event.target.value)}
              />
            </Field>
            <Button
              size="sm"
              variant="secondary"
              className="h-10"
              loading={verifyMutation.isPending}
              disabled={!verifyPath || !verifyPassphrase}
              onClick={() => verifyMutation.mutate()}
            >
              Verify
            </Button>
          </div>
          {verifySummary ? (
            <Alert tone={verifySummary.valid ? "success" : "error"} title="Export verified">
              <div className="space-y-1">
                <div>{verifySummary.message}</div>
                <div className="text-[0.62rem] text-zinc-500">
                  {verifySummary.exportedAt ? `Exported ${verifySummary.exportedAt}` : ""}
                  {verifySummary.schemaVersion !== null
                    ? ` · schema v${verifySummary.schemaVersion}`
                    : ""}
                </div>
              </div>
            </Alert>
          ) : null}
        </div>

        {exports.length > 0 ? (
          <div className="space-y-1.5">
            <SectionLabel>Exports recorded in this vault</SectionLabel>
            {exports.map((backup) => (
              <div
                key={backup.id}
                className="flex flex-wrap items-center gap-2 rounded-lg border border-white/[0.05] px-3 py-2 text-[0.65rem] text-zinc-500"
              >
                <span className="truncate font-mono text-zinc-300">{backup.fileName}</span>
                <span>{formatBytes(backup.byteSize)}</span>
                <span>{formatDate(backup.createdAt.slice(0, 10))}</span>
                <span className="ml-auto font-mono text-zinc-600">
                  {backup.sha256.slice(0, 12)}…
                </span>
              </div>
            ))}
          </div>
        ) : null}
      </Panel>

      <Panel className="space-y-3 p-5">
        <div className="flex items-center gap-2">
          <ShieldCheck className="h-4 w-4 text-sage-400" />
          <SectionLabel>How to restore — read fully before starting</SectionLabel>
        </div>
        <ol className="list-inside list-decimal space-y-1.5 text-[0.7rem] leading-relaxed text-zinc-400">
          <li>Quit Rawjan completely so the database is closed and its write-ahead log is flushed.</li>
          <li>
            Find the live database at <span className="font-mono text-zinc-300">{app.dataDir}\rawjan.db</span>{" "}
            and move it somewhere safe as a second copy.
          </li>
          <li>Copy the chosen backup file into that folder and rename it to <span className="font-mono text-zinc-300">rawjan.db</span>.</li>
          <li>
            Start Rawjan. It opens the file with the key already held in Windows Credential Manager.
            If the key is missing, the backup cannot be read.
          </li>
          <li>Check the dashboard and a few transactions, then create a fresh backup of the restored state.</li>
        </ol>
        <div className="flex items-start gap-2 rounded-xl border border-amber-500/20 bg-amber-500/[0.06] px-3.5 py-2.5 text-[0.65rem] leading-relaxed text-amber-200/90">
          <TriangleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>
            Rawjan will not replace a live database for you. Swapping a file underneath an open
            SQLite connection can corrupt it, so restore stays a deliberate manual step. Validation
            only proves the file opens with the current key; it does not modify anything.
          </span>
        </div>
      </Panel>
    </div>
  );
}
