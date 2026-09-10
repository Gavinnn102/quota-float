import { ArrowClockwise, ArrowDown, ArrowUp, ArrowsInSimple, ArrowsOutSimple, ClockCounterClockwise, CloudSlash, SignIn, WarningCircle } from "@phosphor-icons/react";
import { memo, useEffect, useMemo, useRef, useState } from "react";
import { clampPercent, formatCreditBalance, formatDateTime, formatResetDate, formatResetTime, quotaTier } from "../lib/format";
import { copy, normalizeLanguage } from "../lib/i18n";
import { selectedUsageWindow } from "../lib/snapshots";
import { normalizeQuotaWindow, type Language, type ProviderSnapshot, type QuotaWindow, type WidgetPreferences } from "../types";
import { ProviderMark } from "./ProviderMark";

interface Props {
  snapshot: ProviderSnapshot;
  preferences: WidgetPreferences;
  providerCount: number;
  onPrevious: () => void;
  onNext: () => void;
  onTogglePin: () => void;
  onLanguage: () => void;
  onQuotaWindowToggle?: (quotaWindow: QuotaWindow) => void;
  onHover: (hovered: boolean) => void;
  onRefresh?: () => void;
  isConsuming?: boolean;
  notice?: string | null;
  initialShowCreditTip?: boolean;
  onToggleExpanded: () => void;
  resizeDisabled?: boolean;
}

function StatusIcon({ status, expired = false }: { status: ProviderSnapshot["status"]; expired?: boolean }) {
  if (status === "signed_out") return <SignIn weight="duotone" />;
  if (status === "stale" || expired) return <ClockCounterClockwise weight="duotone" />;
  if (status === "unavailable") return <CloudSlash weight="duotone" />;
  return <WarningCircle weight="duotone" />;
}

function localizedBackendMessage(message: string | null, language: Language): string | null {
  if (!message) return null;
  if (language === "en") return message;
  const normalized = message.toLowerCase();
  if (normalized.includes("sign in") || normalized.includes("login")) return "Codex 登录已失效，请重新登录。";
  if (normalized.includes("rate limited")) return "请求过于频繁，将稍后自动重试。";
  if (normalized.includes("network")) return "网络不可用，将自动重试。";
  if (normalized.includes("format")) return "额度响应格式已变化。";
  if (normalized.includes("missing a quota window")) return "额度响应缺少额度窗口。";
  if (normalized.includes("missing the weekly")) return "额度响应缺少每周窗口。";
  if (normalized.includes("missing the five-hour") || normalized.includes("missing the five hour")) return "额度响应缺少 5 小时窗口。";
  if (normalized.includes("refresh is already running")) return "额度正在刷新，请稍候。";
  return message;
}

function missingWindowMessage(snapshot: ProviderSnapshot, quotaWindow: QuotaWindow, language: Language): string | null {
  if (snapshot.status !== "ok" && snapshot.status !== "stale") return null;
  if (selectedUsageWindow(snapshot, quotaWindow) !== null) return null;
  return copy[normalizeLanguage(language)].windowMissing(quotaWindow);
}

export const QuotaCard = memo(function QuotaCard({
  snapshot,
  preferences,
  providerCount,
  onPrevious,
  onNext,
  onTogglePin: _onTogglePin,
  onLanguage,
  onQuotaWindowToggle = () => {},
  onHover,
  onRefresh,
  isConsuming = false,
  notice = null,
  initialShowCreditTip = false,
  onToggleExpanded,
  resizeDisabled = false,
}: Props) {
  const [showCreditTip, setShowCreditTip] = useState(initialShowCreditTip);
  const language = normalizeLanguage(preferences.language);
  const t = copy[language];
  const quotaWindow = normalizeQuotaWindow(preferences.quotaWindow);
  const selectedWindow = selectedUsageWindow(snapshot, quotaWindow);
  const primary = selectedWindow ? clampPercent(selectedWindow.remainingPercent) : null;
  const staleAge = Date.now() - new Date(snapshot.updatedAt).getTime();
  const staleExpired = snapshot.status === "stale" && staleAge > 30 * 60_000;
  const available = (snapshot.status === "ok" || (snapshot.status === "stale" && !staleExpired)) && selectedWindow !== null;
  const tier = quotaTier(primary);
  const indicatorState = isConsuming && available ? "active" : snapshot.status === "ok" && selectedWindow !== null ? "ok" : snapshot.status === "stale" ? "stale" : "error";
  const indicatorLabel = isConsuming && available
    ? t.active
    : snapshot.status === "ok" && selectedWindow !== null
      ? t.dataSynced
      : snapshot.status === "stale"
        ? t.dataStale
        : snapshot.status === "signed_out"
          ? t.notSignedIn
          : t.unavailableStatus;
  const message = missingWindowMessage(snapshot, quotaWindow, language) ?? localizedBackendMessage(snapshot.message, language);
  const creditExpirations = useMemo(() => (snapshot.resetCreditExpiresAt ?? []).map((value, index) => {
    return t.creditItem(index, formatDateTime(value, language));
  }), [language, snapshot.resetCreditExpiresAt, t]);

  return (
    <main
      className={`quota-card quota-card--${snapshot.status} quota-card--${tier}`}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
    >
      <div className="aurora" aria-hidden="true" />
      <button type="button" className="panel-resize-button" onMouseDown={(event) => event.stopPropagation()} onClick={onToggleExpanded} disabled={resizeDisabled} aria-label={t.collapsePanel} title={t.collapsePanel}>
        <ArrowsInSimple weight="bold" />
      </button>
      <span className="sr-only" aria-live="polite">{available && primary !== null ? t.availableLabel(primary, quotaWindow) : message}</span>
      {notice ? <p className="operation-notice" role="status">{notice}</p> : null}
      <header className="card-header">
        <div>
          <p className="eyebrow">{snapshot.displayName} · {snapshot.plan ?? t.accountFallback} <button type="button" className="quota-window-badge" onMouseDown={(event) => event.stopPropagation()} onClick={() => onQuotaWindowToggle(quotaWindow === "fiveHour" ? "weekly" : "fiveHour")} aria-label={t.switchQuotaWindow(quotaWindow)} title={t.switchQuotaWindow(quotaWindow)}>{t.windowShort(quotaWindow)}</button></p>
          {snapshot.status !== "stale" ? <p className="updated">{t.shortRemaining(quotaWindow)}</p> : null}
        </div>
        <nav className="card-actions" aria-label={t.controls} onMouseDown={(event) => event.stopPropagation()} onMouseUp={(event) => event.stopPropagation()}>
          {providerCount > 1 ? <button onClick={onPrevious} aria-label={t.servicePrevious}><ArrowUp /></button> : null}
          {providerCount > 1 ? <button onClick={onNext} aria-label={t.serviceNext}><ArrowDown /></button> : null}
          <span className={`usage-indicator usage-indicator--${indicatorState}`} role="status" aria-label={indicatorLabel} title={indicatorLabel}><i /></span>
          <button className="language-button" onClick={onLanguage} aria-label={t.switchLanguage} title={t.switchLanguage}>{language === "en" ? "中" : "EN"}</button>
        </nav>
      </header>

      {available && primary !== null ? (
        <>
          <section className="primary-metric" aria-label={t.availableLabel(primary, quotaWindow)}>
            <span>{primary}</span><small>%</small>
          </section>
          <div className="progress" role="progressbar" aria-label={t.availableLabel(primary, quotaWindow)} aria-valuemin={0} aria-valuemax={100} aria-valuenow={primary}>
            <span style={{ width: `${primary}%` }} />
          </div>
          <p className="reset-time">{formatResetTime(selectedWindow.resetsAt, new Date(), language)}</p>
          <footer className="card-footer">
            <div className="weekly-metric">
              <p>{t.windowUntil(quotaWindow, formatResetDate(selectedWindow.resetsAt, language))}</p>
              <p className="credit-balance-row">
                <span>{t.credits}</span>
                <span className="credit-balance-value">{formatCreditBalance(snapshot.credits, language)}</span>
              </p>
              <div className="reset-credit-row" onMouseDown={(event) => event.stopPropagation()}>
                <span>{snapshot.resetCredits === null ? t.resetCreditUnknown : t.resetCredits(snapshot.resetCredits)}</span>
                {snapshot.resetCredits !== null && snapshot.resetCredits > 0 ? (
                  <button type="button" className="reset-credit-button" onClick={() => setShowCreditTip((value) => !value)} aria-expanded={showCreditTip} aria-label={t.view}>{t.view}</button>
                ) : null}
                {showCreditTip ? (
                  <div className="reset-credit-tip" role="status" onMouseDown={(event) => event.stopPropagation()}>
                    {creditExpirations.length > 0 ? creditExpirations.map((item) => <p key={item}>{item}</p>) : <p>{t.noCreditExpiration}</p>}
                  </div>
                ) : null}
              </div>
            </div>
            <ProviderMark />
          </footer>
        </>
      ) : (
        <section className="error-state" aria-live="polite">
          <div className="status-icon" aria-hidden="true"><StatusIcon status={snapshot.status} expired={staleExpired} /></div>
          <strong>{snapshot.status === "signed_out" ? t.signedInRequired : staleExpired ? t.staleExpired : t.temporarilyUnavailable}</strong>
          <p>{message ?? t.errorUnavailable}</p>
          {onRefresh ? (
            <button type="button" className="error-refresh-button" onMouseDown={(event) => event.stopPropagation()} onClick={onRefresh} disabled={!onRefresh} aria-label={t.refreshQuota}>
              <ArrowClockwise />
              <span>{t.refresh}</span>
            </button>
          ) : null}
        </section>
      )}
    </main>
  );
});

export const QuotaOrb = memo(function QuotaOrb({ snapshot, onHover, onToggleExpanded, resizeDisabled = false, notice = null, language = "zh-CN", quotaWindow = "weekly", compactActive = true }: Pick<Props, "snapshot" | "onHover" | "onToggleExpanded" | "resizeDisabled" | "notice"> & { language?: Language; quotaWindow?: QuotaWindow; compactActive?: boolean }) {
  const [idle, setIdle] = useState(false);
  const idleTimer = useRef<number | null>(null);
  const activeLanguage = normalizeLanguage(language);
  const t = copy[activeLanguage];
  const selectedQuotaWindow = normalizeQuotaWindow(quotaWindow);
  const selectedWindow = selectedUsageWindow(snapshot, selectedQuotaWindow);
  const primary = selectedWindow ? clampPercent(selectedWindow.remainingPercent) : null;
  const tier = quotaTier(primary);
  const staleAge = Date.now() - new Date(snapshot.updatedAt).getTime();
  const staleExpired = snapshot.status === "stale" && staleAge > 30 * 60_000;
  const available = (snapshot.status === "ok" || (snapshot.status === "stale" && !staleExpired)) && selectedWindow !== null && primary !== null;
  const message = missingWindowMessage(snapshot, selectedQuotaWindow, activeLanguage) ?? localizedBackendMessage(snapshot.message, activeLanguage);

  useEffect(() => {
    if (idleTimer.current !== null) window.clearTimeout(idleTimer.current);
    setIdle(false);
    if (!compactActive) return;
    idleTimer.current = window.setTimeout(() => setIdle(true), 2000);
    return () => {
      if (idleTimer.current !== null) window.clearTimeout(idleTimer.current);
    };
  }, [compactActive]);

  const handleMouseEnter = () => {
    if (idleTimer.current !== null) window.clearTimeout(idleTimer.current);
    setIdle(false);
    onHover(true);
  };

  return (
    <main
      className={`quota-orb quota-card--${snapshot.status} quota-card--${tier}${idle ? " quota-orb--idle" : ""}`}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={() => onHover(false)}
      aria-label={available ? t.availableLabel(primary, selectedQuotaWindow) : message ?? t.unavailableStatus}
    >
      <div className="aurora" aria-hidden="true" />
      <button type="button" className="panel-resize-button orb-resize-button" onClick={onToggleExpanded} disabled={resizeDisabled} aria-label={t.expandPanel} title={t.expandPanel}>
        <ArrowsOutSimple weight="bold" />
      </button>
      {notice ? <span className="orb-operation-notice" role="status" aria-label={notice} title={notice}><WarningCircle weight="fill" /></span> : null}
      {available ? (
        <div className="orb-metric-wrap">
          <section className="orb-metric" aria-label={t.availableLabel(primary, selectedQuotaWindow)}>
            <span>{primary}</span>
            <small>%</small>
          </section>
          <span className="orb-window-label" aria-hidden="true">{t.windowShort(selectedQuotaWindow)}</span>
        </div>
      ) : (
        <div className="orb-unavailable-wrap">
          <section className="orb-unavailable">
            <StatusIcon status={snapshot.status} expired={staleExpired} />
          </section>
          <span className="orb-window-label orb-window-label--unavailable" aria-hidden="true">{t.windowShort(selectedQuotaWindow)}</span>
        </div>
      )}
    </main>
  );
});
