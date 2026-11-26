import React, { useEffect, useState } from "react";
import { getSystemHealth, checkHealth } from "../api/client";

interface TopBarProps {
  breadcrumb: string[];
  onHelpClick?: () => void;
}

interface StatusState {
  api: "ok" | "err" | "loading";
  core: "ok" | "warn" | "err" | "loading";
}

function useNow() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);
  return now;
}

export const TopBar: React.FC<TopBarProps> = ({ breadcrumb, onHelpClick }) => {
  const [status, setStatus] = useState<StatusState>({ api: "loading", core: "loading" });
  const now = useNow();

  const handleHelp = () => {
    if (onHelpClick) {
      onHelpClick();
    } else {
      window.open("https://github.com/WhatBreaks/WhatBreaks#readme", "_blank", "noopener,noreferrer");
    }
  };

  useEffect(() => {
    let mounted = true;
    const check = async () => {
      try {
        const sys = await getSystemHealth();
        if (!mounted) return;
        const apiOk = sys.components?.api?.status === "healthy";
        const coreStatus = sys.components?.core_engine?.status;
        setStatus({
          api: apiOk ? "ok" : "err",
          core: coreStatus === "healthy" ? "ok" : coreStatus === "degraded" ? "warn" : "err",
        });
      } catch {
        // Fallback to basic health probe if system health endpoint is unavailable
        try {
          const basic = await checkHealth();
          if (!mounted) return;
          const ok = basic.status === "ok" || basic.status === "healthy";
          setStatus({
            api: ok ? "ok" : "err",
            core: "warn", // Honestly warn that core telemetry is unavailable
          });
        } catch {
          if (mounted) setStatus({ api: "err", core: "err" });
        }
      }
    };
    check();
    const interval = setInterval(check, 15_000);
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, []);

  const timeStr = now.toISOString().replace("T", " ").slice(0, 19);

  const apiLabel = status.api === "loading" ? "..." : status.api === "ok" ? "CONNECTED" : "UNREACHABLE";
  const coreLabel = status.core === "loading" ? "..." : status.core === "ok" ? "HEALTHY" : status.core === "warn" ? "DEGRADED" : "UNREACHABLE";

  return (
    <div className="topbar" role="banner">
      {/* Breadcrumbs */}
      <nav className="topbar-breadcrumb" aria-label="Breadcrumb">
        {breadcrumb.map((crumb, i) => (
          <React.Fragment key={i}>
            {i > 0 && <span className="topbar-breadcrumb-sep">›</span>}
            <span className={i === breadcrumb.length - 1 ? "topbar-breadcrumb-active" : ""}>
              {crumb}
            </span>
          </React.Fragment>
        ))}
      </nav>

      {/* Right side: status indicators + clock + help */}
      <div className="topbar-right">
        {/* API status */}
        <div className="topbar-status" aria-label={`API status: ${apiLabel}`}>
          <div className={`topbar-status-dot ${status.api === "ok" ? "ok" : status.api === "loading" ? "warn" : "err"}`} />
          <span className="topbar-status-label">API</span>
          <span className="topbar-status-value">[{apiLabel}]</span>
        </div>

        {/* Core Engine status */}
        <div className="topbar-status" aria-label={`Core Engine status: ${coreLabel}`}>
          <div className={`topbar-status-dot ${status.core === "ok" ? "ok" : status.core === "warn" ? "warn" : "err"}`} />
          <span className="topbar-status-label">Core Engine</span>
          <span className="topbar-status-value">[{coreLabel}]</span>
        </div>

        {/* Clock */}
        <div className="topbar-time" aria-label="Current time">
          {timeStr}
        </div>

        {/* Help */}
        <button
          type="button"
          className="topbar-help"
          aria-label="Documentation and help"
          title="Documentation and help"
          onClick={handleHelp}
        >
          ?
        </button>
      </div>
    </div>
  );
};
