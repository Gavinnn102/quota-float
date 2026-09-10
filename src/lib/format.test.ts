import { describe, expect, it } from "vitest";
import { clampPercent, formatCreditBalance, formatResetDate, formatResetTime, needsFastRefresh, quotaTier } from "./format";

function localDateTime(value: string): string {
  const date = new Date(value);
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${date.getMonth() + 1}/${date.getDate()} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

describe("quota formatting", () => {
  it.each(["zh-CN", "en"] as const)("formats credit units as USD with identical spacing and decimals in %s", (language) => {
    expect(formatCreditBalance({ balance: 1211, unlimited: false }, language)).toBe("US$48.44");
    expect(formatCreditBalance({ balance: 1123, unlimited: false }, language)).toBe("US$44.92");
    expect(formatCreditBalance({ balance: 0, unlimited: false }, language)).toBe("US$0.00");
    expect(formatCreditBalance({ balance: -2.5, unlimited: false }, language)).toBe("US$-0.10");
    expect(formatCreditBalance({ balance: -0.001, unlimited: false }, language)).toBe("US$0.00");
    expect(formatCreditBalance({ balance: 25000, unlimited: false }, language)).toBe("US$1000.00");
    expect(formatCreditBalance({ balance: 1211.24, unlimited: false }, language)).toBe("US$48.45");
  });

  it("does not turn missing or invalid credit balances into zero dollars", () => {
    expect(formatCreditBalance(null)).toBe("—");
    expect(formatCreditBalance(undefined)).toBe("—");
    for (const balance of [null, NaN, Infinity, -Infinity]) {
      expect(formatCreditBalance({ balance, unlimited: false })).toBe("—");
    }
    expect(formatCreditBalance({ balance: null, unlimited: true })).toBe("不限量");
    expect(formatCreditBalance({ balance: 0, unlimited: true }, "en")).toBe("Unlimited");
  });

  it("clamps untrusted percentages", () => {
    expect(clampPercent(-5)).toBe(0);
    expect(clampPercent(51.6)).toBe(52);
    expect(clampPercent(140)).toBe(100);
  });

  it("uses inclusive 50% and 10% quota boundaries", () => {
    expect(quotaTier(50)).toBe("healthy");
    expect(quotaTier(49)).toBe("caution");
    expect(quotaTier(10)).toBe("caution");
    expect(quotaTier(9)).toBe("critical");
    expect(quotaTier(null)).toBe("unknown");
  });

  it("formats reset time in Chinese by default and supports English", () => {
    const now = new Date("2026-07-07T00:00:00Z");
    expect(formatResetTime("2026-07-07T01:30:00Z", now)).toBe("1 小时 30 分钟后重置");
    expect(formatResetTime("2026-07-07T01:30:00Z", now, "zh-CN")).toBe("1 小时 30 分钟后重置");
    expect(formatResetTime("2026-07-07T01:30:00Z", now, "en")).toBe("resets in 1h 30m");
    expect(formatResetTime("2026-07-06T01:00:00Z", now)).toBe("正在更新额度");
    expect(formatResetTime("2026-07-06T01:00:00Z", now, "zh-CN")).toBe("正在更新额度");
    expect(formatResetTime("2026-07-06T01:00:00Z", now, "en")).toBe("Updating quota");
    expect(formatResetTime("invalid", now)).toBe("重置时间未知");
    expect(formatResetTime("invalid", now, "zh-CN")).toBe("重置时间未知");
    expect(formatResetTime("invalid", now, "en")).toBe("Reset time unknown");
  });

  it("accelerates only near a future reset", () => {
    const now = new Date("2026-07-07T00:00:00Z");
    const snapshot = { provider: "codex", displayName: "CODEX", plan: "PRO", weeklyWindow: null, fiveHourWindow: null, credits: null, resetCredits: 0, updatedAt: now.toISOString(), status: "ok", message: null } as const;
    expect(needsFastRefresh({ ...snapshot, weeklyWindow: { remainingPercent: 1, resetsAt: "2026-07-07T00:10:00Z", windowSeconds: 604800 } }, now)).toBe(true);
    expect(needsFastRefresh({ ...snapshot, weeklyWindow: { remainingPercent: 1, resetsAt: "2026-07-07T01:00:00Z", windowSeconds: 604800 } }, now)).toBe(false);
    expect(needsFastRefresh({ ...snapshot, weeklyWindow: { remainingPercent: 1, resetsAt: "2026-07-06T23:58:00Z", windowSeconds: 604800 } }, now)).toBe(true);
  });

  it("also accelerates polling for a near five-hour reset", () => {
    const now = new Date("2026-07-07T00:00:00Z");
    const snapshot = { provider: "codex", displayName: "CODEX", plan: "PRO", weeklyWindow: null, fiveHourWindow: null, credits: null, resetCredits: 0, updatedAt: now.toISOString(), status: "ok", message: null } as const;
    expect(needsFastRefresh({ ...snapshot, fiveHourWindow: { remainingPercent: 22, resetsAt: "2026-07-07T00:12:00Z", windowSeconds: 18_000 } }, now)).toBe(true);
    expect(needsFastRefresh({ ...snapshot, fiveHourWindow: { remainingPercent: 22, resetsAt: "2026-07-07T01:00:00Z", windowSeconds: 18_000 } }, now)).toBe(false);
  });

  it("formats the reset as a compact local date and time", () => {
    const value = "2026-07-10T18:42:00+08:00";
    expect(formatResetDate(value)).toBe(localDateTime(value));
    expect(formatResetDate(value, "en")).toBe(localDateTime(value));
    expect(formatResetDate(null)).toBe("日期未知");
    expect(formatResetDate(null, "zh-CN")).toBe("日期未知");
    expect(formatResetDate(null, "en")).toBe("Date unknown");
  });
});
