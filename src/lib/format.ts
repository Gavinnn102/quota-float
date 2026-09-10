import { copy, normalizeLanguage } from "./i18n";
import type { CreditBalance, Language, ProviderSnapshot } from "../types";

// USD face-value conversion for the current personal Codex credit display.
// The usage endpoint returns credit units, not dollars. See docs/KNOWN-LIMITATIONS.md.
const CREDITS_PER_USD = 25;
const creditAmountFormatter = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
  useGrouping: false,
});

export function formatCreditBalance(credits: CreditBalance | null | undefined, language: Language = "zh-CN"): string {
  if (credits?.unlimited) return copy[normalizeLanguage(language)].creditsUnlimited;
  const balance = credits?.balance;
  if (balance == null || !Number.isFinite(balance)) return "—";
  // The Usage page displays the value backed by complete credits. Fractional
  // credits returned by the usage API must not round the displayed USD value
  // up (for example, 845.5 credits is shown as US$33.80, not US$33.82).
  const displayBalance = balance >= 0 ? Math.floor(balance) : balance;
  const amount = displayBalance / CREDITS_PER_USD;
  return `US$${creditAmountFormatter.format(Math.abs(amount) < 0.005 ? 0 : amount)}`;
}

export function clampPercent(value: number): number {
  return Math.min(100, Math.max(0, Math.round(value)));
}

export function quotaTier(percent: number | null): "unknown" | "healthy" | "caution" | "critical" {
  if (percent === null) return "unknown";
  if (percent >= 50) return "healthy";
  if (percent >= 10) return "caution";
  return "critical";
}

export function formatResetTime(value: string | null, now = new Date(), language: Language = "zh-CN"): string {
  const t = copy[normalizeLanguage(language)];
  if (!value) return t.resetTimeUnknown;
  const target = new Date(value);
  if (Number.isNaN(target.getTime())) return t.resetTimeUnknown;
  const delta = target.getTime() - now.getTime();
  if (delta <= 0) return t.resetUpdating;
  const minutes = Math.ceil(delta / 60_000);
  if (minutes < 60) return t.resetInMinutes(minutes);
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours < 24) return t.resetInHours(hours, rest);
  const days = Math.floor(hours / 24);
  return t.resetInDays(days, hours % 24);
}

export function needsFastRefresh(snapshot: ProviderSnapshot, now = new Date()): boolean {
  return [snapshot.weeklyWindow, snapshot.fiveHourWindow].some((window) => {
    const reset = window?.resetsAt;
    if (!reset) return false;
    const remaining = new Date(reset).getTime() - now.getTime();
    return remaining > -5 * 60_000 && remaining <= 15 * 60_000;
  });
}

export function formatResetDate(value: string | null, language: Language = "zh-CN"): string {
  const t = copy[normalizeLanguage(language)];
  if (!value) return t.dateUnknown;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return t.dateUnknown;
  const parts = new Intl.DateTimeFormat(language === "en" ? "en-US" : "zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).formatToParts(date);
  const getPart = (type: Intl.DateTimeFormatPartTypes) => parts.find((part) => part.type === type)?.value;
  const month = getPart("month");
  const day = getPart("day");
  const hour = getPart("hour");
  const minute = getPart("minute");
  if (!month || !day || !hour || !minute) return t.dateUnknown;
  return `${Number(month)}/${Number(day)} ${hour}:${minute}`;
}

export function formatDateTime(value: string, language: Language): string {
  const t = copy[normalizeLanguage(language)];
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return t.creditExpiresUnknown;
  return new Intl.DateTimeFormat(language === "en" ? "en-US" : "zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(date);
}
