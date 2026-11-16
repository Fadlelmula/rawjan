import { useQuery } from "@tanstack/react-query";
import { Route, Routes } from "react-router-dom";
import { api, errorMessage } from "@/lib/api";
import { BootstrapProvider } from "@/lib/bootstrap";
import { LilyMark } from "@/components/LilyMark";
import { ToastProvider } from "@/components/Toast";
import { Alert, Button } from "@/components/ui";
import { Shell } from "@/components/Shell";
import { DashboardPage } from "@/pages/DashboardPage";
import { TransactionsPage } from "@/pages/TransactionsPage";
import { IncomePage } from "@/pages/IncomePage";
import { AccountsPage } from "@/pages/AccountsPage";
import { BudgetsPage } from "@/pages/BudgetsPage";
import { GoalsPage } from "@/pages/GoalsPage";
import { InvestmentsPage } from "@/pages/InvestmentsPage";
import { LiabilitiesPage } from "@/pages/LiabilitiesPage";
import { NetWorthPage } from "@/pages/NetWorthPage";
import { AnalyticsPage } from "@/pages/AnalyticsPage";
import { CalendarPage } from "@/pages/CalendarPage";
import { JournalPage } from "@/pages/JournalPage";
import { HealthPage } from "@/pages/HealthPage";
import { FiPage } from "@/pages/FiPage";
import { LabPage } from "@/pages/LabPage";
import { ImportsPage } from "@/pages/ImportsPage";
import { BackupsPage } from "@/pages/BackupsPage";
import { SettingsPage } from "@/pages/SettingsPage";

export default function App() {
  const bootstrap = useQuery({
    queryKey: ["bootstrap"],
    queryFn: () => api.bootstrap(),
    retry: 1,
    staleTime: 20_000,
  });

  if (bootstrap.isLoading) {
    return (
      <div className="flex min-h-screen flex-col items-center justify-center gap-5 px-6 text-center">
        <div className="rawjan-pulse flex h-16 w-16 items-center justify-center rounded-2xl border border-lily-500/25 bg-black/40">
          <LilyMark className="h-11 w-11" />
        </div>
        <div>
          <div className="font-display text-xl tracking-[0.02em] text-zinc-100">Rawjan</div>
          <div className="mt-1 text-[0.65rem] uppercase tracking-[0.2em] text-zinc-600">
            Opening encrypted ledger
          </div>
        </div>
      </div>
    );
  }

  if (bootstrap.isError || !bootstrap.data) {
    return (
      <div className="flex min-h-screen items-center justify-center px-6">
        <div className="w-full max-w-lg space-y-4">
          <div className="flex items-center gap-3">
            <div className="flex h-11 w-11 items-center justify-center rounded-xl border border-lily-500/25 bg-black/40">
              <LilyMark className="h-7 w-7" />
            </div>
            <div>
              <div className="font-display text-xl text-zinc-50">Rawjan could not open</div>
              <div className="text-[0.62rem] uppercase tracking-[0.18em] text-zinc-600">
                Local vault unavailable
              </div>
            </div>
          </div>
          <Alert tone="error" title="Startup failed">
            {errorMessage(bootstrap.error)}
          </Alert>
          <div className="rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3 text-[0.7rem] leading-relaxed text-zinc-500">
            The desktop runtime is required. Opening this page in a normal browser cannot reach the local SQLCipher
            database. If the error persists, check the data directory and Windows Credential Manager entry, then
            try again.
          </div>
          <Button variant="primary" onClick={() => void bootstrap.refetch()} loading={bootstrap.isFetching}>
            Try again
          </Button>
        </div>
      </div>
    );
  }

  return (
    <BootstrapProvider value={bootstrap.data}>
      <ToastProvider>
        <Routes>
          <Route element={<Shell />}>
            <Route index element={<DashboardPage />} />
            <Route path="transactions" element={<TransactionsPage />} />
            <Route path="accounts" element={<AccountsPage />} />
            <Route path="budgets" element={<BudgetsPage />} />
            <Route path="savings" element={<GoalsPage />} />
            <Route path="investments" element={<InvestmentsPage />} />
            <Route path="liabilities" element={<LiabilitiesPage />} />
            <Route path="settings" element={<SettingsPage />} />

            <Route path="income" element={<IncomePage />} />
            <Route path="net-worth" element={<NetWorthPage />} />
            <Route path="analytics" element={<AnalyticsPage />} />
            <Route path="calendar" element={<CalendarPage />} />
            <Route path="journal" element={<JournalPage />} />
            <Route path="lab" element={<LabPage />} />
            <Route path="fi" element={<FiPage />} />
            <Route path="health" element={<HealthPage />} />
            <Route path="imports" element={<ImportsPage />} />
            <Route path="backups" element={<BackupsPage />} />
          </Route>
        </Routes>
      </ToastProvider>
    </BootstrapProvider>
  );
}
