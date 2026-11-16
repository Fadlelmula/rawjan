import { createContext, useCallback, useContext, useMemo, useState } from "react";
import type { ReactNode } from "react";
import clsx from "clsx";
import { CircleAlert, CircleCheck, Info, X } from "lucide-react";

type ToastTone = "success" | "error" | "info";

interface ToastItem {
  id: number;
  title: string;
  detail?: string;
  tone: ToastTone;
}

interface ToastApi {
  push: (tone: ToastTone, title: string, detail?: string) => void;
  success: (title: string, detail?: string) => void;
  error: (title: string, detail?: string) => void;
  info: (title: string, detail?: string) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

let nextId = 1;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);

  const dismiss = useCallback((id: number) => {
    setItems((current) => current.filter((item) => item.id !== id));
  }, []);

  const push = useCallback(
    (tone: ToastTone, title: string, detail?: string) => {
      const id = nextId++;
      setItems((current) => [...current.slice(-3), { id, title, detail, tone }]);
      window.setTimeout(() => dismiss(id), tone === "error" ? 9000 : 5000);
    },
    [dismiss],
  );

  const api = useMemo<ToastApi>(
    () => ({
      push,
      success: (title, detail) => push("success", title, detail),
      error: (title, detail) => push("error", title, detail),
      info: (title, detail) => push("info", title, detail),
    }),
    [push],
  );

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div className="pointer-events-none fixed bottom-5 right-5 z-[70] flex w-[min(24rem,calc(100vw-2.5rem))] flex-col gap-2">
        {items.map((item) => (
          <div
            key={item.id}
            className={clsx(
              "pointer-events-auto rawjan-enter flex items-start gap-3 rounded-xl border px-4 py-3 shadow-[0_24px_60px_-24px_rgba(0,0,0,0.9)] backdrop-blur-xl",
              item.tone === "success" && "border-sage-500/30 bg-[#101713]/95 text-sage-200",
              item.tone === "error" && "border-lily-500/40 bg-[#1a0c10]/95 text-lily-100",
              item.tone === "info" && "border-white/10 bg-[#101013]/95 text-zinc-200",
            )}
          >
            <span className="mt-0.5 shrink-0">
              {item.tone === "success" ? <CircleCheck className="h-4 w-4" /> : null}
              {item.tone === "error" ? <CircleAlert className="h-4 w-4" /> : null}
              {item.tone === "info" ? <Info className="h-4 w-4" /> : null}
            </span>
            <div className="min-w-0 flex-1">
              <div className="text-xs font-semibold tracking-wide">{item.title}</div>
              {item.detail ? (
                <div className="mt-1 break-words text-[0.7rem] leading-relaxed opacity-80">
                  {item.detail}
                </div>
              ) : null}
            </div>
            <button
              type="button"
              onClick={() => dismiss(item.id)}
              className="shrink-0 rounded p-0.5 opacity-50 transition hover:opacity-100"
              aria-label="Dismiss"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastApi {
  const context = useContext(ToastContext);
  if (!context) throw new Error("useToast must be used inside ToastProvider");
  return context;
}
