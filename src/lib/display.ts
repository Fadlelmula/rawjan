const MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

const MONTHS_LONG = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

export function parseLocalDate(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (!Number.isFinite(year) || !Number.isFinite(month) || !Number.isFinite(day)) return null;
  const date = new Date(year, month - 1, day);
  // Reject impossible calendar dates such as 2026-13-01 or 2026-02-30, which Date would roll over.
  if (
    date.getFullYear() !== year ||
    date.getMonth() !== month - 1 ||
    date.getDate() !== day
  ) {
    return null;
  }
  return date;
}

export function formatDate(value: string): string {
  const date = parseLocalDate(value);
  if (!date) return value;
  return `${date.getDate()} ${MONTHS[date.getMonth()]} ${date.getFullYear()}`;
}

export function formatDateLong(value: string): string {
  const date = parseLocalDate(value);
  if (!date) return value;
  return `${MONTHS_LONG[date.getMonth()]} ${date.getDate()}, ${date.getFullYear()}`;
}

/** Format an ISO timestamp (with time) in local time, e.g. "16 Nov 2026, 14:03". */
export function formatDateTime(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  const month = MONTHS[parsed.getMonth()];
  const hours = String(parsed.getHours()).padStart(2, "0");
  const minutes = String(parsed.getMinutes()).padStart(2, "0");
  return `${parsed.getDate()} ${month} ${parsed.getFullYear()}, ${hours}:${minutes}`;
}

export function monthLabel(value: string): string {
  const date = parseLocalDate(value);
  if (!date) return value;
  return `${MONTHS[date.getMonth()]} ${date.getFullYear()}`;
}

export function todayInputValue(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

export function money(amount: string, currency: string): string {
  return `${amount} ${currency}`;
}

export function signedAmount(kind: string, amount: string): string {
  switch (kind) {
    case "expense":
    case "fee":
    case "investment_buy":
      return `−${amount}`;
    case "income":
    case "dividend":
    case "refund":
    case "investment_sell":
      return `+${amount}`;
    default:
      return amount;
  }
}

export function amountTone(kind: string): string {
  switch (kind) {
    case "income":
    case "dividend":
    case "refund":
    case "investment_sell":
      return "text-sage-400";
    case "expense":
    case "fee":
    case "investment_buy":
      return "text-zinc-100";
    default:
      return "text-champagne-300";
  }
}

export const KIND_LABELS: Record<string, string> = {
  expense: "Expense",
  income: "Income",
  transfer: "Transfer",
  refund: "Refund",
  fee: "Fee",
  investment_buy: "Investment buy",
  investment_sell: "Investment sell",
  dividend: "Dividend",
  adjustment: "Adjustment",
  opening: "Opening balance",
};

export const STATUS_LABELS: Record<string, string> = {
  draft: "Draft",
  pending: "Pending",
  posted: "Posted",
  reconciled: "Reconciled",
  cancelled: "Cancelled",
  reversed: "Reversed",
};

export function titleCase(value: string): string {
  return value
    .replace(/_/g, " ")
    .replace(/\b\w/g, (character) => character.toUpperCase());
}

export function percentOf(part: number, whole: number): number {
  if (!Number.isFinite(part) || !Number.isFinite(whole) || whole <= 0) return 0;
  return Math.min(100, Math.max(0, (part / whole) * 100));
}
