import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import clsx from "clsx";
import { NotebookPen, Pencil, Plus, Search, Trash2 } from "lucide-react";
import { api, errorMessage } from "@/lib/api";
import type { JournalEntry, JournalInput } from "@/lib/api";
import { useBootstrap } from "@/lib/bootstrap";
import { formatDate, formatDateTime, todayInputValue } from "@/lib/display";
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

interface JournalForm {
  title: string;
  body: string;
  entryDate: string;
}

function blankForm(today: string): JournalForm {
  return { title: "", body: "", entryDate: today };
}

export function JournalPage() {
  const { app } = useBootstrap();
  const queryClient = useQueryClient();
  const toast = useToast();
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState<string | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<JournalEntry | null>(null);
  const [form, setForm] = useState<JournalForm>(() => blankForm(app.today));
  const [formError, setFormError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);

  const entriesQuery = useQuery({
    queryKey: ["journal", search],
    queryFn: () => api.listJournalEntries(search),
  });

  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["journal"] }),
      queryClient.invalidateQueries({ queryKey: ["bootstrap"] }),
    ]);
  };

  const saveMutation = useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: JournalInput }) =>
      api.saveJournalEntry(id, input),
    onSuccess: async () => {
      await refresh();
      setModalOpen(false);
      setEditing(null);
      toast.success(
        editing ? "Entry updated" : "Entry saved",
        "Journal notes stay inside the encrypted local vault.",
      );
    },
    onError: (error) => setFormError(errorMessage(error)),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.deleteJournalEntry(id),
    onSuccess: async () => {
      await refresh();
      toast.success("Entry deleted", "The note was removed from the local vault.");
    },
    onError: (error) => toast.error("Entry not deleted", errorMessage(error)),
  });

  const openCreate = () => {
    setEditing(null);
    setForm(blankForm(app.today));
    setFormError(null);
    setModalOpen(true);
  };

  const openEdit = (entry: JournalEntry) => {
    setEditing(entry);
    setForm({ title: entry.title, body: entry.body, entryDate: entry.entryDate });
    setFormError(null);
    setModalOpen(true);
  };

  const submit = () => {
    if (!form.title.trim()) {
      setFormError("Give the entry a title.");
      return;
    }
    if (!form.entryDate) {
      setFormError("Choose the date the entry belongs to.");
      return;
    }
    setFormError(null);
    saveMutation.mutate({
      id: editing?.id ?? null,
      input: { title: form.title.trim(), body: form.body, entryDate: form.entryDate },
    });
  };

  const entries = entriesQuery.data ?? [];

  return (
    <div className="space-y-4 rawjan-enter">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500">
            Reflection, kept private
          </div>
          <h2 className="mt-1 font-display text-[1.7rem] leading-tight text-zinc-50">Journal</h2>
          <p className="mt-1 max-w-3xl text-xs leading-relaxed text-zinc-500">
            Free-form notes about decisions, trade-offs and plans. Journal entries are not ledger records: they
            never change a balance, and nothing is sent anywhere.
          </p>
        </div>
        <Button size="sm" variant="primary" onClick={openCreate}>
          <Plus className="h-3.5 w-3.5" /> New entry
        </Button>
      </div>

      <Panel className="flex flex-wrap items-center gap-2 p-3">
        <div className="relative min-w-[14rem] flex-1">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-zinc-600" />
          <input
            className="rawjan-input h-9 pl-8 text-xs"
            placeholder="Search titles and notes"
            aria-label="Search journal entries"
            value={searchInput}
            onChange={(event) => setSearchInput(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") setSearch(searchInput.trim() || null);
            }}
          />
        </div>
        <Button
          size="sm"
          variant="secondary"
          onClick={() => setSearch(searchInput.trim() || null)}
        >
          Search
        </Button>
        {search ? (
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              setSearchInput("");
              setSearch(null);
            }}
          >
            Clear
          </Button>
        ) : null}
        <div className="ml-auto text-[0.62rem] text-zinc-600">
          {entries.length} entr{entries.length === 1 ? "y" : "ies"}
          {search ? ` matching “${search}”` : ""}
        </div>
      </Panel>

      {entriesQuery.isError ? (
        <Alert tone="error" title="Could not load the journal">
          {errorMessage(entriesQuery.error)}
        </Alert>
      ) : null}

      {entriesQuery.isLoading ? (
        <div className="flex items-center justify-center py-16 text-zinc-500">
          <Spinner className="h-5 w-5" />
        </div>
      ) : entries.length === 0 ? (
        <EmptyState
          icon={<NotebookPen className="h-5 w-5" />}
          title={search ? "No entries match" : "Nothing written yet"}
          description={
            search
              ? "Try a shorter phrase or clear the search. Only your own entries are searched."
              : "Write down a decision, a reason, or something you want to remember about your money. Entries are dated and stored locally."
          }
          action={
            <Button size="sm" variant="primary" onClick={openCreate}>
              <Plus className="h-3.5 w-3.5" /> Write the first entry
            </Button>
          }
        />
      ) : (
        <div className="grid gap-3 lg:grid-cols-2">
          {entries.map((entry) => {
            const isOpen = expanded === entry.id;
            return (
              <Panel key={entry.id} className="flex flex-col p-5">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <h3 className="truncate font-display text-lg text-zinc-100">{entry.title}</h3>
                      <Badge tone="muted">{formatDate(entry.entryDate)}</Badge>
                    </div>
                    <div className="mt-1 text-[0.58rem] uppercase tracking-[0.14em] text-zinc-600">
                      Updated {formatDateTime(entry.updatedAt)}
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <button
                      type="button"
                      className="rounded-md p-1.5 text-zinc-600 transition hover:bg-white/[0.05] hover:text-zinc-200"
                      onClick={() => openEdit(entry)}
                      aria-label={`Edit ${entry.title}`}
                    >
                      <Pencil className="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      className="rounded-md p-1.5 text-zinc-600 transition hover:bg-white/[0.05] hover:text-lily-300"
                      disabled={deleteMutation.isPending}
                      onClick={() => {
                        if (window.confirm(`Delete “${entry.title}”? This cannot be undone.`)) {
                          deleteMutation.mutate(entry.id);
                        }
                      }}
                      aria-label={`Delete ${entry.title}`}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>

                <div
                  className={clsx(
                    "mt-3 whitespace-pre-wrap text-[0.75rem] leading-relaxed text-zinc-400",
                    !isOpen && "line-clamp-5",
                  )}
                >
                  {entry.body.trim() ? entry.body : "No note body — just a dated heading."}
                </div>

                {entry.body.length > 280 ? (
                  <button
                    type="button"
                    className="mt-2 self-start text-[0.62rem] text-champagne-300/80 transition hover:text-champagne-200"
                    onClick={() => setExpanded(isOpen ? null : entry.id)}
                  >
                    {isOpen ? "Show less" : "Read full entry"}
                  </button>
                ) : null}
              </Panel>
            );
          })}
        </div>
      )}

      <Modal
        open={modalOpen}
        onClose={() => {
          if (!saveMutation.isPending) {
            setModalOpen(false);
            setEditing(null);
          }
        }}
        title={editing ? "Edit journal entry" : "New journal entry"}
        description="A dated note stored only in this vault. It does not post to the ledger."
        width="lg"
        footer={
          <>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                setModalOpen(false);
                setEditing(null);
              }}
              disabled={saveMutation.isPending}
            >
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              loading={saveMutation.isPending}
              onClick={submit}
            >
              {editing ? "Save changes" : "Save entry"}
            </Button>
          </>
        }
      >
        <div className="space-y-3">
          {formError ? (
            <Alert tone="error" title="Entry not saved">
              {formError}
            </Alert>
          ) : null}
          <div className="grid gap-3 sm:grid-cols-[1.6fr_1fr]">
            <Field label="Title">
              <input
                className="rawjan-input"
                value={form.title}
                maxLength={200}
                placeholder="What is this about?"
                onChange={(event) => setForm((current) => ({ ...current, title: event.target.value }))}
              />
            </Field>
            <Field label="Date">
              <input
                type="date"
                className="rawjan-input font-mono"
                value={form.entryDate}
                onChange={(event) =>
                  setForm((current) => ({ ...current, entryDate: event.target.value }))
                }
              />
            </Field>
          </div>
          <Field label="Note" hint="Plain text. Line breaks are preserved.">
            <textarea
              className="rawjan-input min-h-[220px] resize-y leading-relaxed"
              value={form.body}
              placeholder="Write freely…"
              onChange={(event) => setForm((current) => ({ ...current, body: event.target.value }))}
            />
          </Field>
          <div className="text-[0.62rem] leading-relaxed text-zinc-600">
            Editing an entry is recorded in the local audit trail. The original note is not versioned for you.
          </div>
        </div>
      </Modal>

      <div className="text-[0.62rem] text-zinc-700">
        Today is {formatDate(todayInputValue())}. Entries are ordered by date, newest first.
      </div>
    </div>
  );
}
