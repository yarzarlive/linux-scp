import React from "react";
import { Upload, Download, Ban } from "lucide-react";
import { PaneDropState } from "../../hooks/usePaneDragDrop";

interface PaneDropIndicatorProps {
  dropState: PaneDropState;
  itemCount: number;
}

export const PaneDropIndicator: React.FC<PaneDropIndicatorProps> = ({ dropState, itemCount }) => {
  if (dropState.status === "idle") return null;

  if (dropState.status === "valid") {
    const isUpload = dropState.direction === "upload";
    const label = isUpload ? "Upload" : "Download";
    return (
      <div
        role="status"
        data-testid="pane-drop-indicator"
        data-drop-direction={dropState.direction}
        className="flex items-center justify-between gap-2 px-3 py-1.5 bg-emerald-950/90 border-t border-emerald-500/70 text-emerald-100 text-xs select-none"
      >
        <div className="flex items-center gap-2 min-w-0">
          {isUpload ? (
            <Upload className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
          ) : (
            <Download className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
          )}
          <span className="font-semibold shrink-0">
            {label} {itemCount} {itemCount === 1 ? "item" : "items"}
          </span>
          <span className="text-emerald-300/80 shrink-0">→</span>
          <span className="font-mono truncate text-emerald-200" title={dropState.targetDirectory}>
            {dropState.targetDirectory}
          </span>
        </div>
      </div>
    );
  }

  return (
    <div
      role="status"
      data-testid="pane-drop-rejected"
      className="flex items-center gap-2 px-3 py-1.5 bg-rose-950/90 border-t border-rose-500/70 text-rose-200 text-xs select-none"
    >
      <Ban className="w-3.5 h-3.5 text-rose-400 shrink-0" />
      <span className="truncate">{dropState.reason}</span>
    </div>
  );
};
