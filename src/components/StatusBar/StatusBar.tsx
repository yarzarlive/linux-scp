import React from "react";
import { ShieldCheck, HardDrive, Server, ShieldAlert } from "lucide-react";
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
  const elevationInfo =
    activeSession?.elevation.type === "ElevatedSudo" ? activeSession.elevation : null;

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
          className={`text-[10px] uppercase font-mono px-2 py-0.5 rounded border flex items-center gap-1 font-semibold ${
            isElevated
              ? "bg-amber-950/80 border-amber-500 text-amber-300 shadow-sm"
              : "bg-commander-dark border-commander-border text-commander-subtext"
          }`}
          title={
            elevationInfo
              ? `Elevated SFTP active (${elevationInfo.effective_user}): ${elevationInfo.executed_command}`
              : "Standard unprivileged user session"
          }
        >
          {elevationInfo && <ShieldAlert className="w-3 h-3 text-amber-400 shrink-0" />}
          <span>
            {elevationInfo ? `ELEVATED / ROOT (${elevationInfo.effective_user})` : "Elevated: No"}
          </span>
        </div>
      </div>
    </footer>
  );
};
