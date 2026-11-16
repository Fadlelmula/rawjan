import { useMemo, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";
import clsx from "clsx";
import {
  ArrowLeftRight,
  CalendarDays,
  ChartColumn,
  Coins,
  Database,
  FileUp,
  Flame,
  FlaskConical,
  LayoutDashboard,
  Landmark,
  Menu,
  NotebookPen,
  PiggyBank,
  Plus,
  Scale,
  Settings,
  ShieldCheck,
  Sparkles,
  Target,
  TrendingUp,
  Wallet,
  X,
} from "lucide-react";
import { useBootstrap } from "@/lib/bootstrap";
import { formatDateLong } from "@/lib/display";
import { LilyMark } from "@/components/LilyMark";
import { Button } from "@/components/ui";
import { ComposerProvider, useComposerState } from "@/components/ComposerContext";
import { TransactionComposer } from "@/components/TransactionComposer";

interface NavItem {
  to: string;
  label: string;
  icon: typeof LayoutDashboard;
  implemented: boolean;
}

interface NavSection {
  label: string;
  items: NavItem[];
}

const sections: NavSection[] = [
  {
    label: "Overview",
    items: [
      { to: "/", label: "Dashboard", icon: LayoutDashboard, implemented: true },
      { to: "/net-worth", label: "Net worth", icon: Scale, implemented: true },
      { to: "/analytics", label: "Analytics", icon: ChartColumn, implemented: true },
      { to: "/health", label: "Financial health", icon: Flame, implemented: true },
    ],
  },
  {
    label: "Money",
    items: [
      { to: "/income", label: "Income", icon: Coins, implemented: false },
      { to: "/transactions", label: "Transactions", icon: ArrowLeftRight, implemented: true },
      { to: "/accounts", label: "Accounts", icon: Wallet, implemented: true },
      { to: "/budgets", label: "Budgets", icon: Target, implemented: true },
      { to: "/savings", label: "Savings goals", icon: PiggyBank, implemented: true },
      { to: "/investments", label: "Investments", icon: TrendingUp, implemented: true },
      { to: "/liabilities", label: "Liabilities", icon: Landmark, implemented: true },
    ],
  },
  {
    label: "Practice",
    items: [
      { to: "/calendar", label: "Calendar", icon: CalendarDays, implemented: true },
      { to: "/journal", label: "Journal", icon: NotebookPen, implemented: true },
      { to: "/lab", label: "Financial lab", icon: FlaskConical, implemented: true },
      { to: "/fi", label: "FI calculator", icon: Sparkles, implemented: true },
    ],
  },
  {
    label: "Vault",
    items: [
      { to: "/imports", label: "Statement import", icon: FileUp, implemented: true },
      { to: "/backups", label: "Backups & exports", icon: Database, implemented: true },
    ],
  },
];

const allItems = sections.flatMap((section) => section.items);

export function Shell() {
  const { app, tenant, netWorth } = useBootstrap();
  const location = useLocation();
  const [mobileOpen, setMobileOpen] = useState(false);
  const { open, kind, openComposer, setOpen } = useComposerState();

  const title = useMemo(() => {
    if (location.pathname === "/settings") return "Settings";
    return allItems.find((item) => item.to === location.pathname)?.label ?? "Rawjan";
  }, [location.pathname]);

  return (
    <ComposerProvider openComposer={openComposer}>
      <div className="min-h-screen">
        <div
          className={clsx(
            "fixed inset-0 z-30 bg-black/60 backdrop-blur-sm transition-opacity lg:hidden",
            mobileOpen ? "opacity-100" : "pointer-events-none opacity-0",
          )}
          onClick={() => setMobileOpen(false)}
          aria-hidden="true"
        />
        <aside
          className={clsx(
            "fixed inset-y-0 left-0 z-40 flex w-[264px] flex-col border-r border-white/[0.055] bg-ink-900/95 backdrop-blur-xl transition-transform duration-200 lg:translate-x-0",
            mobileOpen ? "translate-x-0" : "-translate-x-full",
          )}
        >
          <div className="flex items-center gap-3 px-5 pb-5 pt-6">
            <div className="relative flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-lily-500/25 bg-black/40">
              <LilyMark className="h-7 w-7" />
            </div>
            <div className="min-w-0">
              <div className="font-display text-[1.15rem] leading-tight tracking-[0.02em] text-zinc-50">
                Rawjan
              </div>
              <div className="truncate text-[0.62rem] uppercase tracking-[0.18em] text-zinc-600">
                {tenant.displayName} · private ledger
              </div>
            </div>
            <button
              type="button"
              className="ml-auto rounded-lg p-1.5 text-zinc-500 lg:hidden"
              onClick={() => setMobileOpen(false)}
              aria-label="Close navigation"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          <div className="rawjan-hairline mx-5 opacity-60" />

          <div className="mx-4 mt-4 rounded-xl border border-white/[0.06] bg-white/[0.02] px-3 py-2.5">
            <div className="text-[0.58rem] font-semibold uppercase tracking-[0.18em] text-zinc-600">
              Net worth
            </div>
            <div className="mt-0.5 font-display text-lg leading-tight text-zinc-100">
              {netWorth.net}{" "}
              <span className="text-[0.62rem] font-sans tracking-wider text-zinc-500">
                {netWorth.currency}
              </span>
            </div>
            <div className="mt-1 flex items-center gap-1 text-[0.6rem] text-zinc-600">
              <ShieldCheck className="h-3 w-3 text-sage-500" />
              {app.databaseEncrypted ? "SQLCipher at rest" : "Unencrypted database"}
            </div>
          </div>

          <nav className="mt-4 flex-1 overflow-y-auto px-3 pb-3">
            {sections.map((section) => (
              <div key={section.label} className="mb-4">
                <div className="px-3 pb-1.5 text-[0.56rem] font-semibold uppercase tracking-[0.2em] text-zinc-600">
                  {section.label}
                </div>
                <div className="space-y-0.5">
                  {section.items.map((item) => (
                    <NavLink
                      key={item.to}
                      to={item.to}
                      end={item.to === "/"}
                      onClick={() => setMobileOpen(false)}
                      className={({ isActive }) =>
                        clsx(
                          "group flex items-center gap-2.5 rounded-lg px-3 py-2 text-[0.8rem] transition-colors",
                          isActive
                            ? "border border-lily-500/20 bg-lily-600/10 text-zinc-50"
                            : "border border-transparent text-zinc-500 hover:bg-white/[0.035] hover:text-zinc-200",
                        )
                      }
                    >
                      {({ isActive }) => (
                        <>
                          <item.icon
                            className={clsx(
                              "h-[15px] w-[15px] shrink-0",
                              isActive ? "text-lily-300" : "text-zinc-600 group-hover:text-zinc-400",
                            )}
                            strokeWidth={1.8}
                          />
                          <span className="flex-1 truncate">{item.label}</span>
                          {!item.implemented ? (
                            <span className="text-[0.5rem] uppercase tracking-[0.12em] text-zinc-700">
                              soon
                            </span>
                          ) : null}
                        </>
                      )}
                    </NavLink>
                  ))}
                </div>
              </div>
            ))}
          </nav>

          <div className="border-t border-white/[0.055] p-3">
            <NavLink
              to="/settings"
              onClick={() => setMobileOpen(false)}
              className={({ isActive }) =>
                clsx(
                  "flex items-center gap-2.5 rounded-lg px-3 py-2 text-[0.8rem] transition-colors",
                  isActive
                    ? "border border-lily-500/20 bg-lily-600/10 text-zinc-50"
                    : "border border-transparent text-zinc-500 hover:bg-white/[0.035] hover:text-zinc-200",
                )
              }
            >
              <Settings className="h-[15px] w-[15px] text-zinc-600" strokeWidth={1.8} />
              <span className="flex-1">Settings</span>
            </NavLink>
            <div className="mt-2 flex items-center justify-between px-3 text-[0.58rem] text-zinc-700">
              <span>Rawjan v{app.version}</span>
              <span>schema {app.schemaVersion}</span>
            </div>
          </div>
        </aside>

        <div className="lg:pl-[264px]">
          <header className="sticky top-0 z-20 flex h-[64px] items-center gap-3 border-b border-white/[0.055] bg-ink-950/80 px-4 backdrop-blur-xl sm:px-6">
            <button
              type="button"
              className="rounded-lg border border-white/10 p-1.5 text-zinc-400 lg:hidden"
              onClick={() => setMobileOpen(true)}
              aria-label="Open navigation"
            >
              <Menu className="h-4 w-4" />
            </button>
            <div className="min-w-0">
              <div className="text-[0.56rem] font-semibold uppercase tracking-[0.2em] text-zinc-600">
                Rawjan · {formatDateLong(app.today)}
              </div>
              <h1 className="truncate font-display text-[1.05rem] leading-tight text-zinc-100">{title}</h1>
            </div>
            <div className="ml-auto flex items-center gap-2">
              <div className="hidden items-center gap-1.5 rounded-full border border-white/[0.07] bg-white/[0.02] px-2.5 py-1 text-[0.62rem] text-zinc-500 sm:flex">
                <ShieldCheck className="h-3 w-3 text-sage-500" />
                Local only
              </div>
              <Button size="sm" variant="primary" onClick={() => openComposer("expense")}>
                <Plus className="h-3.5 w-3.5" />
                New entry
              </Button>
            </div>
          </header>

          <main className="mx-auto w-full max-w-[1460px] px-4 py-5 sm:px-6 sm:py-7">
            <Outlet />
          </main>
        </div>
      </div>

      <TransactionComposer open={open} initialKind={kind} onClose={() => setOpen(false)} />
    </ComposerProvider>
  );
}
