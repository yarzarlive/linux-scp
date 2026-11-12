import React, { useEffect, useRef } from "react";
import { AlertTriangle } from "lucide-react";

interface ConfirmDialogProps {
  isOpen: boolean;
  title: string;
  description: string;
  items?: string[];
  confirmLabel?: string;
  isDestructive?: boolean;
  isLoading?: boolean;
  error?: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  isOpen,
  title,
  description,
  items = [],
  confirmLabel = "Confirm",
  isDestructive = false,
  isLoading = false,
  error = null,
  onConfirm,
  onCancel,
}) => {
  const confirmBtnRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (isOpen) {
      setTimeout(() => {
        confirmBtnRef.current?.focus();
      }, 50);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape" && !isLoading) {
      onCancel();
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 select-none"
      onKeyDown={handleKeyDown}
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-dialog-title"
    >
      <div className="w-full max-w-md rounded-lg border border-commander-border bg-commander-surface shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-100">
        <div className="p-4 border-b border-commander-border flex items-start gap-3">
          <div
            className={`p-2 rounded-full shrink-0 ${
              isDestructive ? "bg-rose-950/60 text-rose-400" : "bg-amber-950/60 text-amber-400"
            }`}
          >
            <AlertTriangle className="w-5 h-5" />
          </div>
          <div className="flex-1 min-w-0">
            <h3 id="confirm-dialog-title" className="text-sm font-semibold text-commander-text">
              {title}
            </h3>
            <p className="text-xs text-commander-subtext/80 mt-1">{description}</p>
          </div>
        </div>

        {items.length > 0 && (
          <div className="px-4 py-3 bg-commander-dark/50 max-h-40 overflow-y-auto border-b border-commander-border/50">
            <p className="text-[11px] font-semibold text-commander-subtext mb-1.5 uppercase tracking-wider">
              Target items ({items.length}):
            </p>
            <ul className="space-y-1 text-xs font-mono text-commander-text">
              {items.map((item, idx) => (
                <li key={idx} className="truncate px-1.5 py-0.5 rounded bg-commander-darker/60">
                  {item}
                </li>
              ))}
            </ul>
          </div>
        )}

        {error && (
          <div className="p-3 text-xs text-rose-400 bg-rose-950/40 border-b border-rose-900/60">
            {error}
          </div>
        )}

        <div className="flex items-center justify-end gap-2 px-4 py-3 bg-commander-darker">
          <button
            type="button"
            onClick={onCancel}
            disabled={isLoading}
            className="px-3 py-1 text-xs rounded border border-commander-border hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            ref={confirmBtnRef}
            type="button"
            onClick={onConfirm}
            disabled={isLoading}
            className={`px-3 py-1 text-xs rounded font-medium transition-colors disabled:opacity-50 cursor-pointer ${
              isDestructive
                ? "bg-rose-600 hover:bg-rose-700 text-white"
                : "bg-commander-accent hover:bg-commander-accent/90 text-commander-dark"
            }`}
          >
            {isLoading ? "Deleting..." : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
};
