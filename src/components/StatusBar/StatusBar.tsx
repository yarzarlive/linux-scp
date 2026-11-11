import React from "react";
import { ShieldCheck, HardDrive } from "lucide-react";
import { SystemInfo } from "../../types/ipc";

interface StatusBarProps {
  systemInfo: SystemInfo | null;
  backendConnected: boolean;
  pingLatencyMs: number | null;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  systemInfo,
  backendConnected,
  pingLatencyMs,
}) => {
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
        <div className="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-commander-dark border border-commander-border">
          Elevated: No
        </div>
      </div>
    </footer>
  );
};
