import { describe, expect, it } from "vitest";
import type { ProviderSnapshot } from "../types";
import { mergeSnapshots, selectedUsageWindow } from "./snapshots";

const success: ProviderSnapshot = {
  provider: "codex",
  displayName: "CODEX",
  plan: "PRO",
  weeklyWindow: { remainingPercent: 42, resetsAt: "2026-07-10T00:00:00Z", windowSeconds: 604_800 },
  fiveHourWindow: { remainingPercent: 78, resetsAt: "2026-07-07T05:00:00Z", windowSeconds: 18_000 },
  credits: { balance: 1211, unlimited: false },
  resetCredits: 1,
  updatedAt: "2026-07-07T00:00:00Z",
  status: "ok",
  message: null,
};

describe("snapshot failure handling", () => {
  it("retains the last successful values during a transient failure", () => {
    const failure: ProviderSnapshot = { ...success, weeklyWindow: null, status: "unavailable", message: "Network unavailable", updatedAt: "2026-07-07T01:00:00Z" };
    expect(mergeSnapshots([success], [failure])[0]).toEqual({ ...success, status: "stale", message: "Network unavailable" });
  });

  it("retains both cached windows during a transient failure", () => {
    const failure: ProviderSnapshot = { ...success, weeklyWindow: null, fiveHourWindow: null, credits: null, status: "unavailable", message: "Network unavailable", updatedAt: "2026-07-07T01:00:00Z" };
    const merged = mergeSnapshots([success], [failure])[0];
    expect(merged.weeklyWindow?.remainingPercent).toBe(42);
    expect(merged.fiveHourWindow?.remainingPercent).toBe(78);
    expect(merged.credits?.balance).toBe(1211);
  });

  it("updates the balance on refresh and clears it when a fresh response omits credits", () => {
    const refreshed: ProviderSnapshot = { ...success, credits: { balance: 1123, unlimited: false } };
    expect(mergeSnapshots([success], [refreshed])[0].credits?.balance).toBe(1123);
    expect(mergeSnapshots([success], [{ ...success, credits: null }])[0].credits).toBeNull();
    expect(mergeSnapshots([success], [{ ...success, credits: null, status: "signed_out" }])[0].credits).toBeNull();
  });

  it("keeps an available five-hour response from falling back to the weekly window", () => {
    const fiveHourOnly: ProviderSnapshot = { ...success, weeklyWindow: null };
    const merged = mergeSnapshots([success], [fiveHourOnly])[0];
    expect(merged.weeklyWindow).toBeNull();
    expect(selectedUsageWindow(merged, "weekly")).toBeNull();
    expect(selectedUsageWindow(merged, "fiveHour")?.remainingPercent).toBe(78);
  });

  it("shows a failure when no successful snapshot exists", () => {
    const signedOut: ProviderSnapshot = { ...success, weeklyWindow: null, status: "signed_out", message: "Please sign in" };
    expect(mergeSnapshots([], [signedOut])[0].status).toBe("signed_out");
  });

  it("does not hide an expired login behind stale quota data", () => {
    const signedOut: ProviderSnapshot = { ...success, weeklyWindow: null, status: "signed_out", message: "Please sign in" };
    expect(mergeSnapshots([success], [signedOut])[0].status).toBe("signed_out");
  });

  it("replaces stale data after recovery", () => {
    expect(mergeSnapshots([{ ...success, status: "stale" }], [{ ...success, weeklyWindow: { ...success.weeklyWindow!, remainingPercent: 88 } }])[0].weeklyWindow?.remainingPercent).toBe(88);
  });
});
