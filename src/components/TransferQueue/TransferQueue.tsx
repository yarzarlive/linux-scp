import React, { useState } from "react";
import { ArrowLeftRight, ChevronDown, ChevronUp, CheckCircle, Clock } from "lucide-react";

interface TransferQueueProps {
  height: number;
  onHeightChange?: (height: number) => void;
  isCollapsed: boolean;
  onToggleCollapse: () => void;
}

export const TransferQueue: React.FC<TransferQueueProps> = ({
  height,
  isCollapsed,
  onToggleCollapse,
}) => {
  const [activeTab, setActiveTab] = useState<"all" | "active" | "queued" | "completed">("all");

  return (
    <section
      role="region"
      aria-label="Transfer Queue"
      className="flex flex-col rounded border border-commander-border bg-commander-surface overflow-hidden select-none"
      style={{ height: isCollapsed ? "auto" : `${height}px` }}
    >
      {/* Transfer Queue Header */}
      <div className="flex items-center justify-between px-3 py-1 bg-commander-darker border-b border-commander-border text-xs">
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={onToggleCollapse}
            className="flex items-center gap-1.5 font-medium text-commander-text hover:text-white transition-colors"
            title={isCollapsed ? "Expand Transfer Queue" : "Collapse Transfer Queue"}
          >
            <ArrowLeftRight className="w-3.5 h-3.5 text-commander-accent" />
            <span>Transfer Queue</span>
            {isCollapsed ? (
              <ChevronUp className="w-3.5 h-3.5 text-commander-subtext" />
            ) : (
              <ChevronDown className="w-3.5 h-3.5 text-commander-subtext" />
            )}
          </button>

          {!isCollapsed && (
            <div className="flex items-center gap-1 text-[11px] text-commander-subtext">
              <button
                type="button"
                onClick={() => setActiveTab("all")}
                className={`px-1.5 py-0.5 rounded transition-colors ${
                  activeTab === "all"
                    ? "bg-commander-dark text-commander-text font-medium"
                    : "hover:text-white"
                }`}
              >
                All (0)
              </button>
              <button
                type="button"
                onClick={() => setActiveTab("active")}
                className={`px-1.5 py-0.5 rounded transition-colors ${
                  activeTab === "active"
                    ? "bg-commander-dark text-commander-text font-medium"
                    : "hover:text-white"
                }`}
              >
                Active (0)
              </button>
              <button
                type="button"
                onClick={() => setActiveTab("queued")}
                className={`px-1.5 py-0.5 rounded transition-colors ${
                  activeTab === "queued"
                    ? "bg-commander-dark text-commander-text font-medium"
                    : "hover:text-white"
                }`}
              >
                Queued (0)
              </button>
              <button
                type="button"
                onClick={() => setActiveTab("completed")}
                className={`px-1.5 py-0.5 rounded transition-colors ${
                  activeTab === "completed"
                    ? "bg-commander-dark text-commander-text font-medium"
                    : "hover:text-white"
                }`}
              >
                Completed (0)
              </button>
            </div>
          )}
        </div>

        <div className="flex items-center gap-3 text-[10px] text-commander-subtext">
          <div className="flex items-center gap-1">
            <Clock className="w-3 h-3 text-commander-accent" />
            <span>0 B/s</span>
          </div>
          <div className="flex items-center gap-1">
            <CheckCircle className="w-3 h-3 text-emerald-400" />
            <span>0 items finished</span>
          </div>
        </div>
      </div>

      {/* Transfer Queue Content Body */}
      {!isCollapsed && (
        <div className="flex-1 flex flex-col items-center justify-center p-4 text-center text-commander-subtext/70 text-xs">
          <ArrowLeftRight className="w-6 h-6 mb-2 opacity-30 text-commander-accent" />
          <p className="font-medium text-commander-subtext">Transfer queue is idle</p>
          <p className="text-[11px] opacity-60 mt-0.5 max-w-sm">
            Active file transfers, throughput rates, and chunked retry progress will be tracked
            here.
          </p>
        </div>
      )}
    </section>
  );
};
