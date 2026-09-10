// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProviderSnapshot, WidgetPreferences } from "./types";

const bridge = vi.hoisted(() => ({
  fetchSnapshots: vi.fn(),
  getPreferences: vi.fn(),
  listenDesktopEvents: vi.fn(),
  setWidgetExpanded: vi.fn(),
  updatePreferences: vi.fn(),
}));

vi.mock("./lib/bridge", () => bridge);

import App from "./App";

const preferences: WidgetPreferences = {
  panelVisible: true,
  expanded: true,
  alwaysOnTop: true,
  pinnedProvider: null,
  autoRotateSeconds: 12,
  language: "en",
  quotaWindow: "weekly",
};

const snapshot: ProviderSnapshot = {
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

beforeEach(() => {
  bridge.fetchSnapshots.mockResolvedValue([snapshot]);
  bridge.getPreferences.mockResolvedValue(preferences);
  bridge.listenDesktopEvents.mockResolvedValue(() => undefined);
  bridge.updatePreferences.mockResolvedValue(undefined);
  window.requestAnimationFrame = (callback: FrameRequestCallback) => window.setTimeout(callback, 0);
  window.cancelAnimationFrame = (handle: number) => window.clearTimeout(handle);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("panel mode transaction", () => {
  it("sends one native transaction for rapid clicks and focuses the new view", async () => {
    let finishResize: ((value: WidgetPreferences) => void) | undefined;
    bridge.setWidgetExpanded.mockImplementation(() => new Promise((resolve) => {
      finishResize = resolve;
    }));

    render(<App />);
    const collapse = await screen.findByRole("button", { name: "Collapse quota panel" });
    fireEvent.click(collapse);
    fireEvent.click(collapse);

    expect(bridge.setWidgetExpanded).toHaveBeenCalledTimes(1);
    expect(bridge.setWidgetExpanded).toHaveBeenCalledWith(false);
    expect(bridge.updatePreferences).not.toHaveBeenCalled();

    await act(async () => {
      finishResize?.({ ...preferences, expanded: false });
    });
    const expand = screen.getByRole("button", { name: "Expand quota panel" });
    await waitFor(() => expect(document.activeElement).toBe(expand));
  });

  it("keeps compact failures visible, restores focus, and does not issue a client rollback", async () => {
    bridge.getPreferences.mockResolvedValue({ ...preferences, expanded: false });
    bridge.setWidgetExpanded.mockRejectedValue("failed to save panel size; previous layout restored");
    render(<App />);

    const expand = await screen.findByRole("button", { name: "Expand quota panel" });
    fireEvent.click(expand);
    await screen.findByLabelText("Panel size change failed. The previous layout was kept.");
    expect(bridge.setWidgetExpanded).toHaveBeenCalledTimes(1);
    expect(bridge.setWidgetExpanded).toHaveBeenCalledWith(true);
    expect(bridge.updatePreferences).not.toHaveBeenCalled();
    await waitFor(() => expect(document.activeElement).toBe(expand));
  });

  it("shows resize failures in the current language and updates an existing notice after a language event", async () => {
    let onPreferences: ((value: WidgetPreferences) => void) | undefined;
    bridge.listenDesktopEvents.mockImplementation(async (handlers: { onPreferences: (value: WidgetPreferences) => void }) => {
      onPreferences = handlers.onPreferences;
      return () => undefined;
    });
    bridge.setWidgetExpanded.mockRejectedValue("failed to save panel size; previous layout restored");
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "Collapse quota panel" }));
    expect(await screen.findByText("Panel size change failed. The previous layout was kept.")).toBeTruthy();

    act(() => onPreferences?.({ ...preferences, language: "zh-CN" }));
    expect(await screen.findByText("额度面板尺寸切换失败，已保留之前的布局。")).toBeTruthy();
    expect(screen.queryByText("Panel size change failed. The previous layout was kept.")).toBeNull();
  });

  it("uses Chinese defaults when settings cannot be read", async () => {
    bridge.getPreferences.mockRejectedValue("settings unavailable");
    render(<App />);

    expect(await screen.findByText("无法读取设置，已使用默认设置。")).toBeTruthy();
  });

  it("keeps both mode controls available while quota data is loading", async () => {
    bridge.fetchSnapshots.mockReturnValue(new Promise(() => undefined));
    bridge.setWidgetExpanded.mockResolvedValue({ ...preferences, expanded: false });
    render(<App />);

    const collapse = await screen.findByRole("button", { name: "Collapse quota panel" });
    expect(await screen.findByRole("button", { name: "Expand quota panel" })).toBeTruthy();
    fireEvent.click(collapse);
    await waitFor(() => expect(bridge.setWidgetExpanded).toHaveBeenCalledWith(false));
  });
});

describe("quota window preference events", () => {
  it("updates the displayed window immediately when the native preference event arrives", async () => {
    let onPreferences: ((value: WidgetPreferences) => void) | undefined;
    bridge.listenDesktopEvents.mockImplementation(async (handlers: { onPreferences: (value: WidgetPreferences) => void }) => {
      onPreferences = handlers.onPreferences;
      return () => undefined;
    });
    render(<App />);

    expect(await screen.findByText("Weekly remaining")).toBeTruthy();
    act(() => onPreferences?.({ ...preferences, quotaWindow: "fiveHour" }));

    expect(await screen.findByText("5-hour remaining")).toBeTruthy();
    expect(screen.getAllByText("75").length).toBeGreaterThan(0);
    expect(screen.getByRole("progressbar", { name: "5-hour quota remaining 75%" })).toBeTruthy();
  });

  it("defaults legacy preferences without a quota window to weekly", async () => {
    const legacy = { ...preferences } as Partial<WidgetPreferences>;
    delete legacy.quotaWindow;
    bridge.getPreferences.mockResolvedValue(legacy);
    render(<App />);

    expect(await screen.findByText("Weekly remaining")).toBeTruthy();
    expect(screen.getAllByText("50").length).toBeGreaterThan(0);
    expect(screen.queryByText("5-hour remaining")).toBeNull();
  });
});

describe("quota polling", () => {
  it("refreshes every minute under normal conditions", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-14T00:00:00Z"));
    render(<App />);

    await act(async () => { await Promise.resolve(); });
    expect(bridge.fetchSnapshots).toHaveBeenCalledTimes(1);

    act(() => vi.advanceTimersByTime(59_999));
    expect(bridge.fetchSnapshots).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(1));
    expect(bridge.fetchSnapshots).toHaveBeenCalledTimes(2);
  });

  it("refreshes every 30 seconds when a quota window is near reset", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-14T00:00:00Z"));
    bridge.fetchSnapshots.mockResolvedValue([{
      ...snapshot,
      fiveHourWindow: { ...snapshot.fiveHourWindow!, resetsAt: "2026-07-14T00:10:00Z" },
    }]);
    render(<App />);

    await act(async () => { await Promise.resolve(); });
    expect(bridge.fetchSnapshots).toHaveBeenCalledTimes(1);

    act(() => vi.advanceTimersByTime(29_999));
    expect(bridge.fetchSnapshots).toHaveBeenCalledTimes(1);
    act(() => vi.advanceTimersByTime(1));
    expect(bridge.fetchSnapshots).toHaveBeenCalledTimes(2);
  });
});
