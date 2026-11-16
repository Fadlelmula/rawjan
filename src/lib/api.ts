import { invoke } from "@tauri-apps/api/core";

export interface MoneyView {
  minor: number;
  currency: string;
  amount: string;
}

export interface AppInfo {
  name: string;
  version: string;
  dataDir: string;
  databaseEncrypted: boolean;
  today: string;
  primaryCurrency: string;
  locale: string;
  schemaVersion: number;
}

export interface PersonInfo {
  id: string;
  displayName: string;
}

export interface Currency {
  code: string;
  name: string;
  symbol: string;
  exponent: number;
  isPrimary: boolean;
  latestRate: string | null;
}

export interface Account {
  id: string;
  name: string;
  accountType: string;
  institution: string | null;
  currency: string;
  currencySymbol: string;
  isLiability: boolean;
  isLiquid: boolean;
  balanceMinor: number;
  balance: string;
  balanceBaseMinor: number | null;
  balanceBase: string | null;
  baseCurrency: string;
  rateMissing: boolean;
  creditLimitMinor: number | null;
  availableMinor: number | null;
  available: string | null;
  statementDay: number | null;
  paymentDueDay: number | null;
  brokerId: string | null;
  notes: string | null;
  color: string | null;
  sortOrder: number;
  isActive: boolean;
  openingBalanceMinor: number;
  openingDate: string | null;
  createdAt: string;
  marketValueMinor: number | null;
}

export interface Category {
  id: string;
  parentId: string | null;
  name: string;
  kind: "income" | "expense" | "equity";
  icon: string | null;
  color: string | null;
  isSystem: boolean;
  isArchived: boolean;
  sortOrder: number;
  usageCount: number;
}

export interface NetWorth {
  asOfDate: string;
  currency: string;
  assetsMinor: number;
  liabilitiesMinor: number;
  netMinor: number;
  assets: string;
  liabilities: string;
  net: string;
  cashMinor: number;
  cash: string;
  investedMinor: number;
  invested: string;
  receivablesMinor: number;
  receivables: string;
  otherAssetsMinor: number;
  otherAssets: string;
  missingRates: string[];
  accountsIncluded: number;
}

export interface NetWorthSnapshot {
  id: string;
  asOfDate: string;
  currency: string;
  assetsMinor: number;
  assets: string;
  liabilitiesMinor: number;
  liabilities: string;
  netMinor: number;
  net: string;
  cashMinor: number;
  cash: string;
  investedMinor: number;
  invested: string;
  source: string;
  createdAt: string;
}

export interface NetWorthOverview {
  current: NetWorth;
  snapshots: NetWorthSnapshot[];
  latestSnapshot: NetWorthSnapshot | null;
  changeMinor: number | null;
  change: string | null;
}

export interface Counts {
  transactions: number;
  accounts: number;
  budgets: number;
  goals: number;
  instruments: number;
  liabilities: number;
  journal: number;
  imports: number;
}

export interface Bootstrap {
  app: AppInfo;
  tenant: PersonInfo;
  user: PersonInfo;
  settings: Record<string, unknown>;
  currencies: Currency[];
  accounts: Account[];
  categories: Category[];
  counts: Counts;
  netWorth: NetWorth;
  onboardingCompleted: boolean;
}

export interface TransactionRow {
  id: string;
  kind: string;
  status: string;
  dataStatus: string;
  description: string;
  payee: string | null;
  notes: string | null;
  localDate: string;
  occurredAt: string;
  accountId: string;
  accountName: string;
  accountCurrency: string;
  counterAccountId: string | null;
  counterAccountName: string | null;
  categoryId: string | null;
  categoryName: string | null;
  incomeSourceId: string | null;
  amountMinor: number;
  amount: string;
  baseMinor: number;
  base: string;
  baseCurrency: string;
  isSplit: boolean;
  recurringRuleId: string | null;
  reversalOfId: string | null;
  transferGroupId: string | null;
  tags: string[];
  splitCount: number;
  attachmentCount: number;
  createdAt: string;
  postedAt: string | null;
  reconciledAt: string | null;
  originalCurrency: string;
  fxRate: string | null;
  fxRateDate: string | null;
}

export interface LedgerEntry {
  id: string;
  entryType: "debit" | "credit";
  amountMinor: number;
  amount: string;
  currency: string;
  baseMinor: number;
  baseAmount: string;
  accountId: string | null;
  accountName: string | null;
  categoryId: string | null;
  categoryName: string | null;
}

export interface TransactionSplit {
  id: string;
  categoryId: string;
  categoryName: string;
  amountMinor: number;
  amount: string;
  note: string | null;
}

export interface TransactionDetail extends Omit<TransactionRow, "tags"> {
  tags: Array<{ id: string; name: string }>;
  splits: TransactionSplit[];
  ledgerEntries: LedgerEntry[];
  externalRef: string | null;
}

export interface PostResult {
  id: string;
  possibleDuplicateId: string | null;
  amount: MoneyView;
  baseAmount: MoneyView;
}

export interface TransactionFilters {
  search?: string;
  accountId?: string;
  categoryId?: string;
  kind?: string;
  status?: string;
  from?: string;
  to?: string;
  limit?: number;
  offset?: number;
}

export interface SplitInput {
  categoryId: string;
  amount: string;
  note?: string | null;
}

export interface TransactionInput {
  kind: string;
  status?: string | null;
  dataStatus?: string | null;
  description: string;
  payee?: string | null;
  notes?: string | null;
  date: string;
  time?: string | null;
  accountId: string;
  counterAccountId?: string | null;
  categoryId?: string | null;
  incomeSourceId?: string | null;
  amount: string;
  currency?: string | null;
  counterAmount?: string | null;
  fxRate?: string | null;
  splits?: SplitInput[] | null;
  tags?: string[] | null;
  externalRef?: string | null;
  direction?: string | null;
}

export interface AccountInput {
  name: string;
  accountType: string;
  institution?: string | null;
  currencyCode: string;
  isLiability?: boolean | null;
  isLiquid?: boolean | null;
  openingBalance?: string | null;
  openingDate?: string | null;
  creditLimit?: string | null;
  statementDay?: number | null;
  paymentDueDay?: number | null;
  brokerId?: string | null;
  notes?: string | null;
  color?: string | null;
  sortOrder?: number | null;
}

export interface ExchangeRate {
  id: string;
  base: string;
  quote: string;
  rate: string;
  asOfDate: string;
  source: string;
  fetchedAt: string;
}

export interface AuditEvent {
  id: string;
  entityType: string;
  entityId: string;
  action: string;
  before: unknown;
  after: unknown;
  createdAt: string;
}

export interface IntegrationStatus {
  id: string;
  name: string;
  category: string;
  available: boolean;
  reason: string;
  documentation: string | null;
}

export interface IncomeSource {
  id: string;
  name: string;
  sourceType: string;
  currency: string;
  expectedAmountMinor: number;
  expectedAmount: string;
  schedule: string;
  payDay: number | null;
  accountId: string | null;
  accountName: string | null;
  categoryId: string | null;
  isActive: boolean;
  notes: string | null;
  receivedBaseMinor: number;
  receivedBase: string;
  baseCurrency: string;
  lastReceivedDate: string | null;
}

export interface BudgetAllocation {
  id: string;
  categoryId: string;
  categoryName: string;
  categoryColor: string | null;
  amountMinor: number;
  amount: string;
  spentMinor: number;
  spent: string;
  remainingMinor: number;
  remaining: string;
  percentUsed: number;
  overBudget: boolean;
  notes: string | null;
  sortOrder: number;
}

export interface Budget {
  id: string;
  name: string;
  method: string;
  periodType: string;
  cycleStartDay: number;
  rolloverPolicy: string;
  currency: string;
  currencySymbol: string;
  alertThresholds: number[];
  isActive: boolean;
  startDate: string;
  allocatedMinor: number;
  allocated: string;
  spentMinor: number;
  spent: string;
  remainingMinor: number;
  remaining: string;
  percentUsed: number;
  periodStart: string;
  periodEnd: string;
  allocations: BudgetAllocation[];
}

export interface BudgetAllocationInput {
  categoryId: string;
  amount: string;
  notes?: string | null;
}

export interface BudgetInput {
  name: string;
  method?: string | null;
  periodType?: string | null;
  cycleStartDay?: number | null;
  rolloverPolicy?: string | null;
  currencyCode?: string | null;
  alertThresholds?: number[] | null;
  startDate?: string | null;
  allocations: BudgetAllocationInput[];
}

export interface GoalContribution {
  id: string;
  amountMinor: number;
  amount: string;
  contributedOn: string;
  accountId: string | null;
  accountName: string | null;
  transactionId: string | null;
  note: string | null;
}

export interface Goal {
  id: string;
  name: string;
  targetAmountMinor: number;
  targetAmount: string;
  currency: string;
  currencySymbol: string;
  targetDate: string | null;
  priority: number;
  accountId: string | null;
  accountName: string | null;
  monthlyContributionMinor: number;
  monthlyContribution: string;
  startDate: string;
  status: string;
  notes: string | null;
  contributedMinor: number;
  contributed: string;
  remainingMinor: number;
  remaining: string;
  percentUsed: number;
  achieved: boolean;
  monthsRemaining: number | null;
  requiredMonthlyMinor: number | null;
  requiredMonthly: string | null;
  contributions: GoalContribution[];
}

export interface GoalInput {
  name: string;
  targetAmount: string;
  currencyCode?: string | null;
  targetDate?: string | null;
  priority?: number | null;
  accountId?: string | null;
  monthlyContribution?: string | null;
  startDate?: string | null;
  status?: string | null;
  notes?: string | null;
}

export interface ContributionInput {
  amount: string;
  contributedOn: string;
  accountId?: string | null;
  note?: string | null;
}

export interface Holding {
  accountId: string;
  accountName: string;
  instrumentId: string;
  symbol: string;
  name: string;
  assetClass: string;
  currency: string;
  quantity: string;
  costBasisMinor: number;
  costBasis: string;
  averageCost: string | null;
  marketValueMinor: number | null;
  marketValue: string | null;
  valuationDate: string | null;
  unrealizedMinor: number | null;
  unrealized: string | null;
  realizedMinor: number;
  realized: string;
}

export interface Instrument {
  id: string;
  symbol: string;
  name: string;
  assetClass: string;
  currency: string;
  currencySymbol: string;
  exchange: string | null;
  isin: string | null;
  notes: string | null;
  tradeCount: number;
  holdingCount: number;
  quantity: string;
  costBasisMinor: number;
  costBasis: string;
  marketValueMinor: number | null;
  marketValue: string | null;
  unrealizedMinor: number | null;
  unrealized: string | null;
  realizedMinor: number;
  realized: string;
  valuedHoldings: number;
  holdings: Holding[];
}

export interface Trade {
  id: string;
  accountId: string;
  accountName: string;
  instrumentId: string;
  symbol: string;
  instrumentName: string;
  side: string;
  tradeDate: string;
  quantity: string;
  price: string;
  feeMinor: number;
  fee: string;
  totalMinor: number;
  total: string;
  currency: string;
  notes: string | null;
}

export interface Valuation {
  id: string;
  accountId: string;
  accountName: string;
  instrumentId: string | null;
  symbol: string | null;
  asOfDate: string;
  valueMinor: number;
  value: string;
  currency: string;
  source: string;
  sourceRef: string | null;
}

export interface CurrencySummary {
  currency: string;
  symbol: string;
  costBasisMinor: number;
  costBasis: string;
  marketValueMinor: number;
  marketValue: string;
  realizedMinor: number;
  realized: string;
  holdings: number;
  valuedHoldings: number;
}

export interface PortfolioSummary {
  instrumentCount: number;
  tradeCount: number;
  valuationCount: number;
  currencies: CurrencySummary[];
}

export interface InstrumentInput {
  symbol: string;
  name: string;
  assetClass: string;
  currencyCode?: string | null;
  exchange?: string | null;
  isin?: string | null;
  notes?: string | null;
}

export interface TradeInput {
  accountId: string;
  instrumentId: string;
  side: string;
  tradeDate: string;
  quantity: string;
  price: string;
  fee?: string | null;
  notes?: string | null;
}

export interface ValuationInput {
  accountId: string;
  instrumentId: string;
  asOfDate: string;
  value: string;
  sourceRef?: string | null;
}

export interface LiabilityPayment {
  id: string;
  paidOn: string;
  amountMinor: number;
  amount: string;
  principalMinor: number;
  principal: string;
  interestMinor: number;
  interest: string;
  feeMinor: number;
  fee: string;
  transactionId: string | null;
  note: string | null;
}

export interface Liability {
  id: string;
  name: string;
  liabilityType: string;
  accountId: string | null;
  accountName: string | null;
  principalMinor: number;
  principal: string;
  outstandingMinor: number;
  outstanding: string;
  currency: string;
  currencySymbol: string;
  interestRateBps: number;
  interestRate: string;
  startDate: string | null;
  termMonths: number | null;
  paymentMinor: number;
  payment: string;
  dueDay: number | null;
  firstPaymentDate: string | null;
  status: string;
  notes: string | null;
  principalRepaidMinor: number;
  principalRepaid: string;
  percentRepaid: number;
  isReceivable: boolean;
  nextDueDate: string | null;
  daysUntilDue: number | null;
  overdue: boolean;
  lastPaymentDate: string | null;
  estimatedPayoffMonths: number | null;
  estimatedPayoffDate: string | null;
  payments: LiabilityPayment[];
}

export interface LiabilityCurrencySummary {
  currency: string;
  symbol: string;
  debtMinor: number;
  debt: string;
  receivableMinor: number;
  receivable: string;
  monthlyPaymentMinor: number;
  monthlyPayment: string;
  activeCount: number;
  overdueCount: number;
}

export interface LiabilitiesSummary {
  totalCount: number;
  activeCount: number;
  paidCount: number;
  currencies: LiabilityCurrencySummary[];
}

export interface LiabilityInput {
  name: string;
  liabilityType: string;
  accountId?: string | null;
  principal: string;
  outstanding: string;
  currencyCode?: string | null;
  interestRate?: string | null;
  startDate?: string | null;
  termMonths?: number | null;
  payment?: string | null;
  dueDay?: number | null;
  firstPaymentDate?: string | null;
  status?: string | null;
  notes?: string | null;
}

export interface LiabilityPaymentInput {
  paidOn: string;
  amount: string;
  principal?: string | null;
  interest?: string | null;
  fee?: string | null;
  note?: string | null;
}

export interface HealthReport {
  status: string;
  integrity: string;
  encrypted: boolean;
  dataDir: string;
}

// ---------------------------------------------------------------------------
// Analytics
// ---------------------------------------------------------------------------

export interface AnalyticsSummary {
  incomeMinor: number;
  income: string;
  expenseMinor: number;
  expense: string;
  netMinor: number;
  net: string;
  savingsRate: number | null;
  transactionCount: number;
  monthCount: number;
  averageMonthlyExpenseMinor: number;
  averageMonthlyExpense: string;
}

export interface MonthPoint {
  month: string;
  label: string;
  incomeMinor: number;
  income: string;
  expenseMinor: number;
  expense: string;
  netMinor: number;
  net: string;
  transactionCount: number;
}

export interface CategorySlice {
  categoryId: string;
  name: string;
  color: string | null;
  amountMinor: number;
  amount: string;
  percent: number;
  transactionCount: number;
}

export interface PayeeRow {
  label: string;
  amountMinor: number;
  amount: string;
  count: number;
}

export interface AnalyticsReport {
  from: string;
  to: string;
  currency: string;
  currencySymbol: string;
  summary: AnalyticsSummary;
  months: MonthPoint[];
  expenseCategories: CategorySlice[];
  incomeCategories: CategorySlice[];
  payees: PayeeRow[];
}

// ---------------------------------------------------------------------------
// Calendar
// ---------------------------------------------------------------------------

export interface CalendarDay {
  date: string;
  day: number;
  incomeMinor: number;
  income: string;
  expenseMinor: number;
  expense: string;
  netMinor: number;
  net: string;
  transactionCount: number;
}

export interface CalendarMonth {
  month: string;
  label: string;
  from: string;
  to: string;
  daysInMonth: number;
  firstWeekday: number;
  currency: string;
  currencySymbol: string;
  incomeMinor: number;
  income: string;
  expenseMinor: number;
  expense: string;
  netMinor: number;
  net: string;
  transactionCount: number;
  activeDays: number;
  days: CalendarDay[];
}

// ---------------------------------------------------------------------------
// Journal
// ---------------------------------------------------------------------------

export interface JournalEntry {
  id: string;
  title: string;
  body: string;
  entryDate: string;
  createdAt: string;
  updatedAt: string;
}

export interface JournalInput {
  title: string;
  body?: string | null;
  entryDate: string;
}

// ---------------------------------------------------------------------------
// Financial health
// ---------------------------------------------------------------------------

export interface FinancialHealthComponent {
  key: string;
  label: string;
  weight: number;
  score: number | null;
  available: boolean;
  detail: string;
  metric: string | null;
}

export interface FinancialHealthReport {
  score: number;
  grade: string;
  label: string;
  currency: string;
  components: FinancialHealthComponent[];
  cash: string;
  debt: string;
  assets: string;
  invested: string;
  averageMonthlyExpense: string | null;
  savingsRate: number | null;
  emergencyMonths: string | null;
  debtRatio: number | null;
  investedShare: number | null;
  netWorthTrend: number | null;
  budgetCount: number;
  budgetsWithin: number;
  notes: string[];
}

// ---------------------------------------------------------------------------
// FI calculator
// ---------------------------------------------------------------------------

export interface FiInput {
  annualExpenses: string;
  withdrawalRate: string;
  currentInvested: string;
  monthlyContribution: string;
  expectedReturn: string;
  inflation: string;
  yearsToRetirement?: number | null;
}

export interface FiYearPoint {
  year: number;
  balanceMinor: number;
  balance: string;
  targetMinor: number;
  target: string;
}

export interface FiReport {
  currency: string;
  currencySymbol: string;
  fiNumberMinor: number;
  fiNumber: string;
  progressPercent: number;
  remainingMinor: number;
  remaining: string;
  monthsToFi: number | null;
  yearsToFi: string | null;
  fiDate: string | null;
  realReturnPercent: string;
  coastNumberMinor: number | null;
  coastNumber: string | null;
  coastProgressPercent: number | null;
  projectedAtRetirement: string | null;
  annualExpenses: string;
  currentInvested: string;
  monthlyContribution: string;
  projection: FiYearPoint[];
  notes: string[];
}

// ---------------------------------------------------------------------------
// Financial lab
// ---------------------------------------------------------------------------

export interface CompoundInput {
  initial: string;
  monthlyContribution: string;
  annualReturn: string;
  years: number;
}

export interface CompoundPoint {
  year: number;
  balanceMinor: number;
  balance: string;
  contributedMinor: number;
  contributed: string;
}

export interface CompoundReport {
  currency: string;
  currencySymbol: string;
  finalMinor: number;
  finalAmount: string;
  contributedMinor: number;
  contributed: string;
  growthMinor: number;
  growth: string;
  points: CompoundPoint[];
  notes: string[];
}

export interface LoanInput {
  principal: string;
  annualRate: string;
  months: number;
}

export interface LoanRow {
  month: number;
  paymentMinor: number;
  payment: string;
  interestMinor: number;
  interest: string;
  principalMinor: number;
  principal: string;
  balanceMinor: number;
  balance: string;
}

export interface LoanReport {
  currency: string;
  currencySymbol: string;
  monthlyPaymentMinor: number;
  monthlyPayment: string;
  totalPaidMinor: number;
  totalPaid: string;
  totalInterestMinor: number;
  totalInterest: string;
  months: number;
  rows: LoanRow[];
  notes: string[];
}

export interface InflationInput {
  amount: string;
  annualInflation: string;
  years: number;
}

export interface InflationReport {
  currency: string;
  currencySymbol: string;
  futureCostMinor: number;
  futureCost: string;
  realValueMinor: number;
  realValue: string;
  increaseMinor: number;
  increase: string;
  notes: string[];
}

export interface ColumnMapping {
  date: string | null;
  description: string | null;
  amount: string | null;
  debit: string | null;
  credit: string | null;
  payee: string | null;
  category: string | null;
  currency: string | null;
  reference: string | null;
  dateOrder: string;
  decimalSeparator: string;
  defaultKind: string;
  skipDuplicates: boolean;
  status: string;
}

export interface ImportPreviewRow {
  rowIndex: number;
  cells: string[];
  normalized: Record<string, unknown> | null;
  status: string;
  issue: string | null;
}

export interface ImportPreview {
  jobId: string;
  fileName: string;
  format: string;
  fileHash: string;
  rowCount: number;
  headers: string[];
  rows: ImportPreviewRow[];
  suggestedMapping: ColumnMapping;
  warnings: string[];
  accountId: string | null;
}

export interface ImportJob {
  id: string;
  fileName: string;
  format: string;
  status: string;
  rowCount: number;
  importedCount: number;
  duplicateCount: number;
  errorCount: number;
  accountId: string | null;
  createdAt: string;
  committedAt: string | null;
}

export interface ImportRowIssue {
  rowIndex: number;
  message: string;
}

export interface ImportResult {
  jobId: string;
  imported: number;
  duplicates: number;
  errors: number;
  skipped: number;
  issues: ImportRowIssue[];
}

export interface Backup {
  id: string;
  path: string;
  fileName: string;
  kind: string;
  byteSize: number;
  sha256: string;
  encrypted: boolean;
  schemaVersion: number;
  verifiedAt: string | null;
  createdAt: string;
}

export interface TableCount {
  table: string;
  rows: number;
}

export interface BackupValidation {
  path: string;
  fileName: string;
  readable: boolean;
  encrypted: boolean;
  schemaVersion: number | null;
  integrity: string;
  tables: TableCount[];
  message: string;
}

export interface ExportResult {
  id: string;
  path: string;
  fileName: string;
  byteSize: number;
  sha256: string;
  tableCounts: TableCount[];
  createdAt: string;
}

export interface ExportSummary {
  path: string;
  exportedAt: string | null;
  schemaVersion: number | null;
  tables: TableCount[];
  valid: boolean;
  message: string;
}

export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  health: () => invoke<HealthReport>("health"),
  listTransactions: (filters: TransactionFilters = {}) =>
    invoke<{ rows: TransactionRow[]; total: number }>("list_transactions", {
      search: filters.search ?? null,
      accountId: filters.accountId ?? null,
      categoryId: filters.categoryId ?? null,
      kind: filters.kind ?? null,
      status: filters.status ?? null,
      from: filters.from ?? null,
      to: filters.to ?? null,
      limit: filters.limit ?? 100,
      offset: filters.offset ?? 0,
    }),
  getTransaction: (id: string) => invoke<TransactionDetail>("get_transaction", { id }),
  createTransaction: (input: TransactionInput) =>
    invoke<PostResult>("create_transaction", { input }),
  updateTransaction: (id: string, input: TransactionInput) =>
    invoke<void>("update_transaction", { id, input }),
  setTransactionStatus: (id: string, status: string) =>
    invoke<void>("set_transaction_status", { id, status }),
  reverseTransaction: (id: string) => invoke<string>("reverse_transaction", { id }),
  deleteTransaction: (id: string) => invoke<void>("delete_transaction", { id }),
  checkDuplicate: (input: {
    accountId: string;
    date: string;
    amount: string;
    description: string;
    kind: string;
  }) => invoke<{ possibleDuplicateId: string | null }>("check_duplicate", { input }),
  listAccounts: (includeArchived = false) =>
    invoke<Account[]>("list_accounts", { includeArchived }),
  createAccount: (input: AccountInput) => invoke<string>("create_account", { input }),
  updateAccount: (id: string, input: AccountInput) =>
    invoke<void>("update_account", { id, input }),
  archiveAccount: (id: string, archive: boolean) =>
    invoke<void>("archive_account", { id, archive }),
  accountRegister: (accountId: string, limit = 100, offset = 0) =>
    invoke<{ rows: TransactionRow[]; total: number; account: Account }>("account_register", {
      accountId,
      limit,
      offset,
    }),
  listCategories: (includeArchived = false) =>
    invoke<Category[]>("list_categories_cmd", { includeArchived }),
  listIncomeSources: () => invoke<IncomeSource[]>("list_income_sources"),
  updateSetting: (key: string, value: unknown) =>
    invoke<unknown>("update_setting", { input: { key, value } }),
  settingsMap: () => invoke<Record<string, unknown>>("settings_map"),
  listExchangeRates: (baseCode?: string) =>
    invoke<ExchangeRate[]>("list_exchange_rates", { baseCode: baseCode ?? null }),
  addExchangeRate: (baseCode: string, quoteCode: string, rate: string, asOfDate: string) =>
    invoke<ExchangeRate>("add_exchange_rate", { baseCode, quoteCode, rate, asOfDate }),
  listAuditEvents: (limit = 50) => invoke<AuditEvent[]>("list_audit_events", { limit }),
  listIntegrationStatus: () => invoke<IntegrationStatus[]>("list_integration_status"),
  listBudgets: (reference?: string) =>
    invoke<Budget[]>("list_budgets", { reference: reference ?? null }),
  saveBudget: (id: string | null, input: BudgetInput) =>
    invoke<string>("save_budget", { id, input }),
  setBudgetActive: (id: string, active: boolean) =>
    invoke<void>("set_budget_active", { id, active }),
  deleteBudget: (id: string) => invoke<void>("delete_budget", { id }),
  listGoals: (reference?: string) =>
    invoke<Goal[]>("list_goals", { reference: reference ?? null }),
  saveGoal: (id: string | null, input: GoalInput) =>
    invoke<string>("save_goal", { id, input }),
  addGoalContribution: (goalId: string, input: ContributionInput) =>
    invoke<string>("add_goal_contribution", { goalId, input }),
  deleteGoalContribution: (id: string) =>
    invoke<void>("delete_goal_contribution", { id }),
  setGoalStatus: (id: string, status: string) =>
    invoke<void>("set_goal_status", { id, status }),
  deleteGoal: (id: string) => invoke<void>("delete_goal", { id }),
  listInstruments: () => invoke<Instrument[]>("list_instruments"),
  portfolioSummary: () => invoke<PortfolioSummary>("portfolio_summary"),
  saveInstrument: (id: string | null, input: InstrumentInput) =>
    invoke<string>("save_instrument", { id, input }),
  deleteInstrument: (id: string) => invoke<void>("delete_instrument", { id }),
  listTrades: (instrumentId?: string) =>
    invoke<Trade[]>("list_trades", { instrumentId: instrumentId ?? null }),
  saveTrade: (id: string | null, input: TradeInput) =>
    invoke<string>("save_trade", { id, input }),
  deleteTrade: (id: string) => invoke<void>("delete_trade", { id }),
  listValuations: (instrumentId?: string) =>
    invoke<Valuation[]>("list_valuations", { instrumentId: instrumentId ?? null }),
  saveValuation: (id: string | null, input: ValuationInput) =>
    invoke<string>("save_valuation", { id, input }),
  deleteValuation: (id: string) => invoke<void>("delete_valuation", { id }),
  listLiabilities: () => invoke<Liability[]>("list_liabilities"),
  liabilitiesSummary: () => invoke<LiabilitiesSummary>("liabilities_summary"),
  saveLiability: (id: string | null, input: LiabilityInput) =>
    invoke<string>("save_liability", { id, input }),
  saveLiabilityPayment: (liabilityId: string, id: string | null, input: LiabilityPaymentInput) =>
    invoke<string>("save_liability_payment", { liabilityId, id, input }),
  deleteLiabilityPayment: (id: string) =>
    invoke<void>("delete_liability_payment", { id }),
  setLiabilityStatus: (id: string, status: string) =>
    invoke<void>("set_liability_status", { id, status }),
  deleteLiability: (id: string) => invoke<void>("delete_liability", { id }),
  netWorthOverview: () => invoke<NetWorthOverview>("net_worth_overview"),
  captureNetWorthSnapshot: () =>
    invoke<NetWorthSnapshot>("capture_net_worth_snapshot"),
  deleteNetWorthSnapshot: (id: string) =>
    invoke<void>("delete_net_worth_snapshot", { id }),
  analyticsReport: (from?: string | null, to?: string | null) =>
    invoke<AnalyticsReport>("analytics_report", { from: from ?? null, to: to ?? null }),
  monthCalendar: (month?: string | null) =>
    invoke<CalendarMonth>("month_calendar", { month: month ?? null }),
  listJournalEntries: (search?: string | null, limit = 200) =>
    invoke<JournalEntry[]>("list_journal_entries", { search: search ?? null, limit }),
  saveJournalEntry: (id: string | null, input: JournalInput) =>
    invoke<string>("save_journal_entry", { id, input }),
  deleteJournalEntry: (id: string) => invoke<void>("delete_journal_entry", { id }),
  financialHealth: () => invoke<FinancialHealthReport>("financial_health"),
  fiCalculate: (input: FiInput) => invoke<FiReport>("fi_calculate", { input }),
  labCompound: (input: CompoundInput) => invoke<CompoundReport>("lab_compound", { input }),
  labLoan: (input: LoanInput) => invoke<LoanReport>("lab_loan", { input }),
  labInflation: (input: InflationInput) =>
    invoke<InflationReport>("lab_inflation", { input }),
  importPreview: (path: string, format: string, accountId: string | null) =>
    invoke<ImportPreview>("import_preview", { path, format, accountId }),
  importCommit: (
    jobId: string,
    mapping: ColumnMapping,
    headers: string[],
    accountId: string | null,
  ) =>
    invoke<ImportResult>("import_commit", { jobId, mapping, headers, accountId }),
  listImportJobs: () => invoke<ImportJob[]>("list_import_jobs"),
  cancelImport: (jobId: string) => invoke<void>("cancel_import", { jobId }),
  createBackup: (kind?: string) =>
    invoke<Backup>("create_backup", { kind: kind ?? null }),
  listBackups: () => invoke<Backup[]>("list_backups"),
  deleteBackup: (id: string) => invoke<void>("delete_backup", { id }),
  validateBackup: (path: string) =>
    invoke<BackupValidation>("validate_backup", { path }),
  createEncryptedExport: (passphrase: string) =>
    invoke<ExportResult>("create_encrypted_export", { passphrase }),
  verifyEncryptedExport: (path: string, passphrase: string) =>
    invoke<ExportSummary>("verify_encrypted_export", { path, passphrase }),
};

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return "Something went wrong. No changes were saved.";
}
