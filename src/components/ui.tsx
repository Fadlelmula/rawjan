import type { ButtonHTMLAttributes, ReactNode } from "react";
import { useEffect, useRef } from "react";
import clsx from "clsx";
import { X } from "lucide-react";

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "quiet";
type ButtonSize = "sm" | "md";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
}

const buttonVariants: Record<ButtonVariant, string> = {
  primary:
    "bg-lily-500 text-white shadow-[0_10px_30px_-14px_rgba(195,54,77,0.85)] hover:bg-lily-400 border border-lily-400/60",
  secondary:
    "bg-white/[0.045] text-zinc-100 border border-white/10 hover:bg-white/[0.075] hover:border-white/20",
  ghost: "bg-transparent text-zinc-300 border border-transparent hover:bg-white/[0.05] hover:text-white",
  danger:
    "bg-lily-600/20 text-lily-200 border border-lily-500/40 hover:bg-lily-600/30 hover:text-white",
  quiet: "bg-transparent text-zinc-500 border border-transparent hover:text-zinc-200",
};

const buttonSizes: Record<ButtonSize, string> = {
  sm: "h-8 px-3 text-xs rounded-lg gap-1.5",
  md: "h-10 px-4 text-sm rounded-xl gap-2",
};

export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  className,
  children,
  disabled,
  ...rest
}: ButtonProps) {
  return (
    <button
      className={clsx(
        "inline-flex select-none items-center justify-center font-medium tracking-[0.01em] transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50",
        buttonSizes[size],
        buttonVariants[variant],
        className,
      )}
      disabled={disabled || loading}
      {...rest}
    >
      {loading ? <Spinner className="h-3.5 w-3.5" /> : null}
      {children}
    </button>
  );
}

export function Spinner({ className }: { className?: string }) {
  return (
    <span
      className={clsx(
        "inline-block animate-spin rounded-full border-2 border-current border-t-transparent",
        className ?? "h-4 w-4",
      )}
      aria-hidden="true"
    />
  );
}

export function Panel({
  className,
  children,
  ...rest
}: { className?: string; children: ReactNode } & React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={clsx("rawjan-panel", className)} {...rest}>
      {children}
    </div>
  );
}

export function SectionLabel({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div
      className={clsx(
        "text-[0.63rem] font-semibold uppercase tracking-[0.18em] text-zinc-500",
        className,
      )}
    >
      {children}
    </div>
  );
}

type BadgeTone = "default" | "red" | "green" | "amber" | "muted" | "champagne";

const badgeTones: Record<BadgeTone, string> = {
  default: "border-white/10 bg-white/[0.05] text-zinc-300",
  red: "border-lily-500/40 bg-lily-600/15 text-lily-200",
  green: "border-sage-500/40 bg-sage-500/10 text-sage-400",
  amber: "border-amber-500/30 bg-amber-500/10 text-amber-300",
  muted: "border-white/[0.06] bg-transparent text-zinc-500",
  champagne: "border-champagne-500/30 bg-champagne-500/10 text-champagne-300",
};

export function Badge({
  children,
  tone = "default",
  className,
}: {
  children: ReactNode;
  tone?: BadgeTone;
  className?: string;
}) {
  return (
    <span
      className={clsx(
        "inline-flex items-center gap-1 whitespace-nowrap rounded-full border px-2 py-0.5 text-[0.67rem] font-medium tracking-wide",
        badgeTones[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}

export function Field({
  label,
  hint,
  error,
  children,
  className,
}: {
  label: string;
  hint?: string;
  error?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <label className={clsx("block", className)}>
      <span className="rawjan-label">{label}</span>
      {children}
      {hint && !error ? <span className="mt-1 block text-[0.68rem] text-zinc-500">{hint}</span> : null}
      {error ? <span className="mt-1 block text-[0.68rem] text-lily-300">{error}</span> : null}
    </label>
  );
}

export function Modal({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  width = "md",
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
  width?: "sm" | "md" | "lg" | "xl";
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const opener = document.activeElement as HTMLElement | null;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
      // Keep Tab focus inside the dialog.
      if (event.key === "Tab" && dialogRef.current) {
        const focusable = Array.from(
          dialogRef.current.querySelectorAll<HTMLElement>(
            'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
          ),
        );
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        const active = document.activeElement;
        const inside = dialogRef.current.contains(active);
        if (event.shiftKey && (!inside || active === first)) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && (!inside || active === last)) {
          event.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", onKeyDown);
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = previous;
      opener?.focus?.();
    };
  }, [open, onClose]);

  if (!open) return null;

  const widths = {
    sm: "max-w-md",
    md: "max-w-xl",
    lg: "max-w-3xl",
    xl: "max-w-5xl",
  };

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/70 p-4 backdrop-blur-sm sm:p-8">
      <div
        ref={dialogRef}
        className={clsx(
          "rawjan-panel rawjan-enter my-auto w-full overflow-hidden",
          widths[width],
        )}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="flex items-start justify-between gap-4 border-b border-white/[0.06] px-5 py-4">
          <div>
            <h2 className="font-display text-xl text-zinc-50">{title}</h2>
            {description ? (
              <p className="mt-1 max-w-xl text-xs leading-relaxed text-zinc-500">{description}</p>
            ) : null}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-transparent p-1.5 text-zinc-500 transition hover:border-white/10 hover:bg-white/5 hover:text-white"
            aria-label="Close"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="px-5 py-5">{children}</div>
        {footer ? (
          <div className="flex items-center justify-end gap-2 border-t border-white/[0.06] bg-black/20 px-5 py-3.5">
            {footer}
          </div>
        ) : null}
      </div>
    </div>
  );
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon?: ReactNode;
  title: string;
  description: string;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={clsx(
        "flex flex-col items-center justify-center rounded-2xl border border-dashed border-white/10 px-6 py-12 text-center",
        className,
      )}
    >
      {icon ? (
        <div className="mb-4 flex h-11 w-11 items-center justify-center rounded-xl border border-white/10 bg-white/[0.03] text-lily-300">
          {icon}
        </div>
      ) : null}
      <h3 className="font-display text-lg text-zinc-100">{title}</h3>
      <p className="mt-1.5 max-w-md text-sm leading-relaxed text-zinc-500">{description}</p>
      {action ? <div className="mt-5">{action}</div> : null}
    </div>
  );
}

export function Alert({
  tone = "info",
  title,
  children,
  action,
}: {
  tone?: "info" | "warning" | "error" | "success";
  title?: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  const tones = {
    info: "border-white/10 bg-white/[0.03] text-zinc-300",
    warning: "border-amber-500/25 bg-amber-500/[0.07] text-amber-200",
    error: "border-lily-500/30 bg-lily-600/10 text-lily-200",
    success: "border-sage-500/30 bg-sage-500/[0.08] text-sage-300",
  };
  return (
    <div className={clsx("flex items-start justify-between gap-4 rounded-xl border px-4 py-3", tones[tone])}>
      <div className="text-xs leading-relaxed">
        {title ? <div className="mb-0.5 text-[0.7rem] font-semibold uppercase tracking-wider">{title}</div> : null}
        {children}
      </div>
      {action}
    </div>
  );
}
