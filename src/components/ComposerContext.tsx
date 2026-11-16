import { createContext, useCallback, useContext, useMemo, useState } from "react";
import type { ReactNode } from "react";

interface ComposerApi {
  openComposer: (kind?: string) => void;
}

const ComposerContext = createContext<ComposerApi | null>(null);

export function ComposerProvider({
  openComposer,
  children,
}: {
  openComposer: (kind?: string) => void;
  children: ReactNode;
}) {
  const value = useMemo(() => ({ openComposer }), [openComposer]);
  return <ComposerContext.Provider value={value}>{children}</ComposerContext.Provider>;
}

export function useComposer(): ComposerApi {
  const context = useContext(ComposerContext);
  if (!context) throw new Error("useComposer must be used inside ComposerProvider");
  return context;
}

export function useComposerState() {
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState("expense");
  const openComposer = useCallback((nextKind?: string) => {
    if (nextKind) setKind(nextKind);
    setOpen(true);
  }, []);
  return { open, kind, openComposer, setOpen };
}
