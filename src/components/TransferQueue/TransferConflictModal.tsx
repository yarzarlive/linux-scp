import React, { useState } from "react";
import { AlertTriangle, FileWarning, X } from "lucide-react";
import { ConflictDecision, TransferConflictPrompt } from "../../types/transfer";
import { formatFileSize, formatTimestamp } from "../../utils/formatters";

interface TransferConflictModalProps {
  conflict: TransferConflictPrompt | null;
  onResolve: (conflictId: string, decision: ConflictDecision, applyToAll: boolean) => void;
}

export const TransferConflictModal: React.FC<TransferConflictModalProps> = ({
  conflict,
  onResolve,
}) => {
  const [applyToAll, setApplyToAll] = useState(false);

  if (!conflict) return null;

  const choose = (decision: ConflictDecision) => {
    const apply = applyToAll;
    setApplyToAll(false);
    onResolve(conflict.conflict_id, decision, apply);
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="File Conflict"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 select-none"
    >
      <div className="w-full max-w-lg rounded-lg border border-amber-700/60 bg-commander-darker shadow-2xl overflow-hidden text-commander-text">
        <div className="flex items-center justify-between px-4 py-2.5 bg-amber-950/50 border-b border-amber-800/50">
          <div className="flex items-center gap-2 text-amber-300 font-semibold text-xs">
            <FileWarning className="w-4 h-4 text-amber-400" />
            <span>Destination File Already Exists</span>
          </div>
          <button
            type="button"
            onClick={() => choose("cancel")}
            className="text-commander-subtext hover:text-white p-0.5 rounded"
            title="Cancel Transfer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-4 space-y-3 text-xs">
          <p className="text-commander-subtext">
            A file already exists at the destination during{" "}
            <span className="text-commander-text font-medium">{conflict.direction}</span>. Choose
            how to proceed:
          </p>

          <div className="grid grid-cols-1 gap-2">
            <div className="p-2.5 rounded bg-commander-surface border border-commander-border space-y-1">
              <div className="text-[11px] font-semibold text-commander-accent">Source</div>
              <div className="font-mono text-[11px] break-all select-text">
                {conflict.source_path}
              </div>
              <div className="flex items-center gap-4 text-[11px] text-commander-subtext">
                <span>Size: {formatFileSize(conflict.source_size, false)}</span>
                <span>Modified: {formatTimestamp(conflict.source_modified_at_secs)}</span>
              </div>
            </div>

            <div className="p-2.5 rounded bg-commander-surface border border-commander-border space-y-1">
              <div className="text-[11px] font-semibold text-amber-300 flex items-center gap-1">
                <AlertTriangle className="w-3 h-3" />
                <span>Existing Destination</span>
              </div>
              <div className="font-mono text-[11px] break-all select-text">
                {conflict.destination_path}
              </div>
              <div className="flex items-center gap-4 text-[11px] text-commander-subtext">
                <span>Size: {formatFileSize(conflict.target_size, false)}</span>
                <span>Modified: {formatTimestamp(conflict.target_modified_at_secs)}</span>
              </div>
            </div>
          </div>

          <label className="flex items-center gap-2 text-xs text-commander-subtext cursor-pointer pt-1">
            <input
              type="checkbox"
              checked={applyToAll}
              onChange={(e) => setApplyToAll(e.target.checked)}
              className="rounded border-commander-border bg-commander-dark text-commander-accent"
            />
            <span>Apply this choice to all remaining conflicts in this transfer</span>
          </label>
        </div>

        <div className="flex flex-wrap items-center justify-end gap-2 px-4 py-2.5 bg-commander-surface border-t border-commander-border text-xs">
          <button
            type="button"
            onClick={() => choose("overwrite")}
            className="px-3 py-1.5 rounded bg-amber-600/20 hover:bg-amber-600/30 border border-amber-600/50 text-amber-200 font-medium transition-colors"
          >
            Overwrite
          </button>
          <button
            type="button"
            onClick={() => choose("overwrite_if_newer")}
            className="px-3 py-1.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text transition-colors"
          >
            Overwrite if Newer
          </button>
          <button
            type="button"
            onClick={() => choose("rename")}
            className="px-3 py-1.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text transition-colors"
          >
            Rename
          </button>
          <button
            type="button"
            onClick={() => choose("skip")}
            className="px-3 py-1.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-commander-text transition-colors"
          >
            Skip
          </button>
          <button
            type="button"
            onClick={() => choose("cancel")}
            className="px-3 py-1.5 rounded bg-rose-950/50 hover:bg-rose-900/60 border border-rose-800/60 text-rose-200 transition-colors"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};
