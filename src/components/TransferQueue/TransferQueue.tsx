import React, { useMemo, useState } from "react";
import {
  ArrowLeftRight,
  ArrowUp,
  ArrowDown,
  ChevronDown,
  ChevronUp,
  CheckCircle,
  Clock,
  X,
  RotateCcw,
  Trash2,
  Folder,
  AlertCircle,
} from "lucide-react";
import { TransferJob } from "../../types/transfer";
import { formatFileSize } from "../../utils/formatters";

interface TransferQueueProps {
  jobs?: TransferJob[];
  height: number;
  onHeightChange?: (height: number) => void;
  isCollapsed: boolean;
  onToggleCollapse: () => void;
  onCancelJob?: (jobId: string) => void;
  onRetryJob?: (jobId: string) => void;
  onClearFinished?: () => void;
}

function formatSpeed(bps: number): string {
  if (!bps || bps <= 0) return "0 B/s";
  return `${formatFileSize(bps, false)}/s`;
}

function formatEta(seconds?: number): string {
  if (seconds === undefined || seconds === null) return "—";
  if (seconds < 60) return `${seconds}s`;
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  if (mins < 60) return `${mins}m ${secs.toString().padStart(2, "0")}s`;
  const hrs = Math.floor(mins / 60);
  const remMins = mins % 60;
  return `${hrs}h ${remMins.toString().padStart(2, "0")}m`;
}

function isJobActive(job: TransferJob): boolean {
  return (
    job.state.status === "Connecting" ||
    job.state.status === "Transferring" ||
    job.state.status === "Verifying"
  );
}

export const TransferQueue: React.FC<TransferQueueProps> = ({
  jobs = [],
  height,
  isCollapsed,
  onToggleCollapse,
  onCancelJob,
  onRetryJob,
  onClearFinished,
}) => {
  const [activeTab, setActiveTab] = useState<"all" | "active" | "queued" | "completed">("all");

  const counts = useMemo(() => {
    let active = 0;
    let queued = 0;
    let completed = 0;
    let totalSpeedBps = 0;

    for (const job of jobs) {
      if (isJobActive(job)) {
        active++;
        totalSpeedBps += job.speed_bps || 0;
      } else if (job.state.status === "Queued") {
        queued++;
      } else if (job.state.status === "Completed") {
        completed++;
      }
    }

    return {
      all: jobs.length,
      active,
      queued,
      completed,
      totalSpeedBps,
    };
  }, [jobs]);

  const filteredJobs = useMemo(() => {
    switch (activeTab) {
      case "active":
        return jobs.filter(isJobActive);
      case "queued":
        return jobs.filter((j) => j.state.status === "Queued");
      case "completed":
        return jobs.filter((j) => j.state.status === "Completed");
      default:
        return jobs;
    }
  }, [jobs, activeTab]);

  const hasFinished = jobs.some(
    (j) => j.state.status === "Completed" || j.state.status === "Cancelled"
  );

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
                All ({counts.all})
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
                Active ({counts.active})
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
                Queued ({counts.queued})
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
                Completed ({counts.completed})
              </button>
            </div>
          )}
        </div>

        <div className="flex items-center gap-3 text-[10px] text-commander-subtext">
          {hasFinished && onClearFinished && (
            <button
              type="button"
              onClick={onClearFinished}
              className="flex items-center gap-1 px-1.5 py-0.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-subtext hover:text-white transition-colors"
              title="Clear Completed & Cancelled"
            >
              <Trash2 className="w-3 h-3" />
              <span>Clear Finished</span>
            </button>
          )}
          <div className="flex items-center gap-1">
            <Clock className="w-3 h-3 text-commander-accent" />
            <span>{formatSpeed(counts.totalSpeedBps)}</span>
          </div>
          <div className="flex items-center gap-1">
            <CheckCircle className="w-3 h-3 text-emerald-400" />
            <span>{counts.completed} items finished</span>
          </div>
        </div>
      </div>

      {/* Transfer Queue Content Body */}
      {!isCollapsed && (
        <div className="flex-1 overflow-y-auto min-h-0">
          {filteredJobs.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center p-4 text-center text-commander-subtext/70 text-xs">
              <ArrowLeftRight className="w-6 h-6 mb-2 opacity-30 text-commander-accent" />
              <p className="font-medium text-commander-subtext">Transfer queue is idle</p>
              <p className="text-[11px] opacity-60 mt-0.5 max-w-sm">
                Active file transfers, throughput rates, and chunked retry progress will be tracked
                here.
              </p>
            </div>
          ) : (
            <div className="divide-y divide-commander-border/40">
              {filteredJobs.map((job) => {
                const active = isJobActive(job);
                const canCancel = active || job.state.status === "Queued";
                const canRetry = job.state.status === "Failed" || job.state.status === "Cancelled";
                const total = job.total_bytes;
                const pct =
                  total !== undefined && total !== null
                    ? total === 0
                      ? 100
                      : Math.min(100, Math.round((job.transferred_bytes / total) * 100))
                    : null;

                return (
                  <div
                    key={job.id}
                    data-testid={`transfer-job-${job.id}`}
                    className="flex items-center gap-3 px-3 py-1.5 text-xs hover:bg-commander-dark/40"
                  >
                    {/* Direction Icon */}
                    <div className="shrink-0 flex items-center gap-1">
                      {job.direction === "upload" ? (
                        <ArrowUp className="w-3.5 h-3.5 text-sky-400" />
                      ) : (
                        <ArrowDown className="w-3.5 h-3.5 text-emerald-400" />
                      )}
                      {job.is_directory && <Folder className="w-3.5 h-3.5 text-amber-400" />}
                    </div>

                    {/* Source -> Destination + Directory File Info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-1.5 truncate font-mono text-[11px] text-commander-text">
                        <span className="truncate" title={job.source_path}>
                          {job.source_path}
                        </span>
                        <span className="text-commander-subtext shrink-0">→</span>
                        <span
                          className="truncate text-commander-subtext"
                          title={job.destination_path}
                        >
                          {job.destination_path}
                        </span>
                      </div>

                      {job.state.status === "Failed" ? (
                        <div className="flex items-center gap-1 text-[10px] text-rose-400 truncate mt-0.5">
                          <AlertCircle className="w-3 h-3 shrink-0" />
                          <span className="truncate">{job.state.details.error_message}</span>
                        </div>
                      ) : job.is_directory && (job.total_files ?? 0) > 0 ? (
                        <div className="text-[10px] text-commander-subtext truncate mt-0.5">
                          {job.completed_files ?? 0}/{job.total_files} files
                          {job.current_file ? ` • ${job.current_file}` : ""}
                        </div>
                      ) : null}
                    </div>

                    {/* Progress Bar */}
                    <div className="w-36 shrink-0">
                      <div className="w-full h-1.5 rounded bg-commander-dark overflow-hidden border border-commander-border/50">
                        <div
                          className={`h-full transition-all ${
                            job.state.status === "Completed"
                              ? "bg-emerald-500"
                              : job.state.status === "Failed"
                                ? "bg-rose-500"
                                : job.state.status === "Cancelled"
                                  ? "bg-amber-500"
                                  : "bg-commander-accent"
                          }`}
                          style={{ width: `${pct ?? (active ? 50 : 0)}%` }}
                        />
                      </div>
                      <div className="flex justify-between text-[10px] font-mono text-commander-subtext mt-0.5">
                        <span>
                          {formatFileSize(job.transferred_bytes, false)}
                          {total !== undefined && total !== null
                            ? ` / ${formatFileSize(total, false)}`
                            : ""}
                        </span>
                        <span>{pct !== null ? `${pct}%` : "—"}</span>
                      </div>
                    </div>

                    {/* Speed & ETA */}
                    <div className="w-28 shrink-0 text-right font-mono text-[10px] text-commander-subtext">
                      <div>{active ? formatSpeed(job.speed_bps) : job.state.status}</div>
                      {active && <div>ETA: {formatEta(job.eta_seconds)}</div>}
                    </div>

                    {/* Actions */}
                    <div className="w-16 shrink-0 flex items-center justify-end gap-1">
                      {canCancel && onCancelJob && (
                        <button
                          type="button"
                          onClick={() => onCancelJob(job.id)}
                          className="p-1 rounded hover:bg-rose-950/60 text-commander-subtext hover:text-rose-300 transition-colors"
                          title="Cancel Transfer"
                        >
                          <X className="w-3.5 h-3.5" />
                        </button>
                      )}
                      {canRetry && onRetryJob && (
                        <button
                          type="button"
                          onClick={() => onRetryJob(job.id)}
                          className="p-1 rounded hover:bg-commander-selected text-commander-subtext hover:text-commander-accent transition-colors"
                          title="Retry Transfer"
                        >
                          <RotateCcw className="w-3.5 h-3.5" />
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}
    </section>
  );
};
