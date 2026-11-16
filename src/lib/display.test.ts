import { describe, expect, it } from "vitest";
import {
  formatDate,
  formatDateLong,
  monthLabel,
  parseLocalDate,
  percentOf,
  signedAmount,
  titleCase,
} from "./display";

describe("parseLocalDate", () => {
  it("reads an ISO date as a local calendar date", () => {
    const date = parseLocalDate("2026-02-28");
    expect(date).not.toBeNull();
    expect(date?.getFullYear()).toBe(2026);
    expect(date?.getMonth()).toBe(1);
    expect(date?.getDate()).toBe(28);
  });

  it("rejects anything that is not a full ISO date", () => {
    expect(parseLocalDate("2026-2-8")).toBeNull();
    expect(parseLocalDate("28/02/2026")).toBeNull();
    expect(parseLocalDate("")).toBeNull();
  });
});

describe("date labels", () => {
  it("formats short, long and month labels without shifting the day", () => {
    expect(formatDate("2026-01-01")).toBe("1 Jan 2026");
    expect(formatDateLong("2026-12-31")).toBe("December 31, 2026");
    expect(monthLabel("2026-07-15")).toBe("Jul 2026");
  });

  it("returns the original value when it cannot be parsed", () => {
    expect(formatDate("not a date")).toBe("not a date");
    expect(monthLabel("2026-13-01")).toBe("2026-13-01");
  });
});

describe("signedAmount", () => {
  it("adds a minus to outflow kinds", () => {
    expect(signedAmount("expense", "12.50")).toBe("−12.50");
    expect(signedAmount("fee", "3.00")).toBe("−3.00");
    expect(signedAmount("investment_buy", "500.00")).toBe("−500.00");
  });

  it("adds a plus to inflow kinds", () => {
    expect(signedAmount("income", "5000.00")).toBe("+5000.00");
    expect(signedAmount("refund", "20.00")).toBe("+20.00");
    expect(signedAmount("dividend", "7.25")).toBe("+7.25");
  });

  it("leaves neutral kinds untouched", () => {
    expect(signedAmount("transfer", "100.00")).toBe("100.00");
    expect(signedAmount("adjustment", "4.00")).toBe("4.00");
  });
});

describe("titleCase", () => {
  it("turns snake_case into readable labels", () => {
    expect(titleCase("investment_buy")).toBe("Investment Buy");
    expect(titleCase("pre_migration")).toBe("Pre Migration");
  });
});

describe("percentOf", () => {
  it("clamps shares to a sane geometry range", () => {
    expect(percentOf(25, 100)).toBe(25);
    expect(percentOf(200, 100)).toBe(100);
    expect(percentOf(-5, 100)).toBe(0);
  });

  it("returns zero when the whole is not positive", () => {
    expect(percentOf(10, 0)).toBe(0);
    expect(percentOf(10, Number.NaN)).toBe(0);
  });
});
