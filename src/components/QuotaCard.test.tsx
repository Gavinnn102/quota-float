// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ProviderSnapshot, WidgetPreferences } from "../types";
import { QuotaCard, QuotaOrb } from "./QuotaCard";

const baseSnapshot: ProviderSnapshot = {
  provider: "codex",
  displayName: "CODEX",
  plan: "PRO",
  weeklyWindow: {
    remainingPercent: 50,
    resetsAt: "2026-07-21T00:00:00Z",
    windowSeconds: 604_800,
  },
  fiveHourWindow: {
    remainingPercent: 75,
    resetsAt: "2026-07-14T05:00:00Z",
    windowSeconds: 18_000,
  },
  resetCredits: 0,
  updatedAt: "2026-07-14T00:00:00Z",
  status: "ok",
  message: null,
};

const preferences: WidgetPreferences = {
  panelVisible: true,
  expanded: true,
  alwaysOnTop: true,
  pinnedProvider: null,
  autoRotateSeconds: 12,
  language: "zh-CN",
  quotaWindow: "weekly",
};

function localDateTime(value: string): string {
  const date = new Date(value);
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${date.getMonth() + 1}/${date.getDate()} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function renderOrb(snapshot: ProviderSnapshot, quotaWindow: WidgetPreferences["quotaWindow"] = "weekly") {
  return render(
    <QuotaOrb
      snapshot={snapshot}
      language="zh-CN"
      quotaWindow={quotaWindow}
      onHover={() => undefined}
      onToggleExpanded={() => undefined}
    />,
  );
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("compact quota view", () => {
  it.each([0, 50, 100])("renders the exact %s%% boundary without hiding it", (remainingPercent) => {
    renderOrb({
      ...baseSnapshot,
      weeklyWindow: { ...baseSnapshot.weeklyWindow!, remainingPercent },
    });
    expect(screen.getByText(String(remainingPercent))).toBeTruthy();
    expect(screen.getByRole("button", { name: "展开额度面板" })).toBeTruthy();
  });

  it("uses the selected five-hour window in the orb and exposes its short label", () => {
    renderOrb(baseSnapshot, "fiveHour");
    expect(screen.getByText("75")).toBeTruthy();
    expect(screen.getByText("5h")).toBeTruthy();
    expect(screen.getByRole("main", { name: "5 小时额度剩余 75%" })).toBeTruthy();
  });

  it("keeps a fresh stale percentage consistent with the expanded panel", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-14T00:20:00Z"));
    renderOrb({ ...baseSnapshot, status: "stale" });
    expect(screen.getByText("50")).toBeTruthy();
  });

  it("stops showing a stale percentage after the 30 minute accuracy boundary", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-14T00:31:00Z"));
    renderOrb({ ...baseSnapshot, status: "stale" });
    expect(screen.queryByText("50")).toBeNull();
  });

  it.each(["signed_out", "unavailable"] as const)("shows an error glyph for %s", (status) => {
    renderOrb({ ...baseSnapshot, status, weeklyWindow: null, message: `${status} message` });
    expect(screen.queryByText("50")).toBeNull();
    expect(screen.getByLabelText(`${status} message`)).toBeTruthy();
  });

  it("starts the idle fade only after the compact view becomes active", () => {
    vi.useFakeTimers();
    const props = {
      snapshot: baseSnapshot,
      language: "zh-CN" as const,
      onHover: () => undefined,
      onToggleExpanded: () => undefined,
    };
    const { container, rerender } = render(<QuotaOrb {...props} compactActive={false} />);
    act(() => vi.advanceTimersByTime(2500));
    expect(container.querySelector(".quota-orb")?.classList.contains("quota-orb--idle")).toBe(false);

    rerender(<QuotaOrb {...props} compactActive />);
    act(() => vi.advanceTimersByTime(2000));
    expect(container.querySelector(".quota-orb")?.classList.contains("quota-orb--idle")).toBe(true);
  });

});

describe("manual recovery", () => {
  it.each(["signed_out", "unavailable"] as const)("offers refresh for %s failures", (status) => {
    const onRefresh = vi.fn();
    render(
      <QuotaCard
        snapshot={{ ...baseSnapshot, status, weeklyWindow: null, message: `${status} message` }}
        preferences={preferences}
        providerCount={1}
        onPrevious={() => undefined}
        onNext={() => undefined}
        onTogglePin={() => undefined}
        onLanguage={() => undefined}
        onHover={() => undefined}
        onRefresh={onRefresh}
        onToggleExpanded={() => undefined}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "刷新额度数据" }));
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });
});

describe("selected quota window", () => {
  it("uses the five-hour reset source and its color tier", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-14T00:00:00Z"));
    const fiveHourCritical = {
      ...baseSnapshot,
      fiveHourWindow: { ...baseSnapshot.fiveHourWindow!, remainingPercent: 8, resetsAt: "2026-07-14T05:00:00Z" },
    };
    const { container } = render(
      <QuotaCard
        snapshot={fiveHourCritical}
        preferences={{ ...preferences, language: "en", quotaWindow: "fiveHour" }}
        providerCount={1}
        onPrevious={() => undefined}
        onNext={() => undefined}
        onTogglePin={() => undefined}
        onLanguage={() => undefined}
        onHover={() => undefined}
        onToggleExpanded={() => undefined}
      />,
    );

    expect(screen.getByText("8")).toBeTruthy();
    expect(screen.getByText("resets in 5h")).toBeTruthy();
    expect(screen.getByText(`5-hour remaining · until ${localDateTime(fiveHourCritical.fiveHourWindow!.resetsAt!)}`)).toBeTruthy();
    expect(container.querySelector(".quota-card--critical")).toBeTruthy();
  });

  it("does not substitute the weekly window when the selected five-hour window is missing", () => {
    render(
      <QuotaCard
        snapshot={{ ...baseSnapshot, fiveHourWindow: null }}
        preferences={{ ...preferences, language: "en", quotaWindow: "fiveHour" }}
        providerCount={1}
        onPrevious={() => undefined}
        onNext={() => undefined}
        onTogglePin={() => undefined}
        onLanguage={() => undefined}
        onHover={() => undefined}
        onRefresh={() => undefined}
        onToggleExpanded={() => undefined}
        isConsuming
      />,
    );

    expect(screen.getAllByText("5-hour quota window is unavailable. Try refreshing shortly.").length).toBeGreaterThan(0);
    expect(screen.queryByText("50")).toBeNull();
    expect(screen.queryByText("75")).toBeNull();
    expect(screen.queryByLabelText("Quota in use")).toBeNull();
  });
});
