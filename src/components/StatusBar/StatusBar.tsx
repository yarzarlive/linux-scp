import React from "react";
import { ShieldCheck, HardDrive, Server } from "lucide-react";
import { SystemInfo } from "../../types/ipc";
import { ConnectionSession } from "../../types/session";

interface StatusBarProps {
  systemInfo: SystemInfo | null;
  backendConnected: boolean;
  pingLatencyMs: number | null;
  activeSession?: ConnectionSession | null;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  systemInfo,
  backendConnected,
  pingLatencyMs,
  activeSession = null,
}) => {
  const isElevated = activeSession?.elevation.type === "ElevatedSudo";

  return (
    <footer className="flex items-center justify-between px-3 py-1 bg-commander-surface border-t border-commander-border text-[11px] text-commander-subtext select-none">
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-1.5">
          <span
            className={`w-2 h-2 rounded-full ${
              backendConnected ? "bg-emerald-500" : "bg-rose-500 animate-pulse"
            }`}
          />
          <span>{backendConnected ? "Backend Online" : "Backend Disconnected"}</span>
          {pingLatencyMs !== null && <span>({pingLatencyMs}ms)</span>}
        </div>
        <div className="flex items-center gap-1">
          <ShieldCheck className="w-3.5 h-3.5 text-emerald-400" />
          <span>Strict Host-Key Verification</span>
        </div>
        {activeSession && (
          <div className="flex items-center gap-1.5 text-emerald-300 font-mono">
            <Server className="w-3.5 h-3.5 text-emerald-400" />
            <span>
              {activeSession.protocol}: {activeSession.username}@{activeSession.host}:
              {activeSession.port}
            </span>
          </div>
        )}
      </div>
      <div className="flex items-center gap-4">
        {systemInfo && (
          <div className="flex items-center gap-1.5">
            <HardDrive className="w-3.5 h-3.5" />
            <span>
              {systemInfo.os_name} ({systemInfo.arch})
            </span>
          </div>
        )}
        <div
          className={`text-[10px] uppercase font-mono px-1.5 py-0.5 rounded border ${
            isElevated
              ? "bg-amber-950/60 border-amber-700 text-amber-300"
              : "bg-commander-dark border-commander-border"
          }`}
        >
          Elevated: {isElevated ? "Yes" : "No"}
        </div>
      </div>
    </footer>
  );
};
