import React, { useState, useEffect, useRef } from "react";
import { AlertCircle } from "lucide-react";

interface InputDialogProps {
  isOpen: boolean;
  title: string;
  description?: string;
  initialValue?: string;
  placeholder?: string;
  confirmLabel?: string;
  isLoading?: boolean;
  error?: string | null;
  onConfirm: (value: string) => void;
  onCancel: () => void;
}

export const InputDialog: React.FC<InputDialogProps> = ({
  isOpen,
  title,
  description,
  initialValue = "",
  placeholder = "",
  confirmLabel = "Confirm",
  isLoading = false,
  error = null,
  onConfirm,
  onCancel,
}) => {
  const [value, setValue] = useState(initialValue);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setValue(initialValue);
      setTimeout(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
      }, 50);
    }
  }, [isOpen, initialValue]);

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (value.trim() && !isLoading) {
      onConfirm(value.trim());
    }
  };

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
      aria-labelledby="dialog-title"
    >
      <div className="w-full max-w-md rounded-lg border border-commander-border bg-commander-surface shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-100">
        <form onSubmit={handleSubmit}>
          <div className="p-4 border-b border-commander-border">
            <h3 id="dialog-title" className="text-sm font-semibold text-commander-text">
              {title}
            </h3>
            {description && <p className="text-xs text-commander-subtext/80 mt-1">{description}</p>}
          </div>

          <div className="p-4 space-y-3">
            <input
              ref={inputRef}
              type="text"
              value={value}
              onChange={(e) => setValue(e.target.value)}
              placeholder={placeholder}
              disabled={isLoading}
              className="w-full px-3 py-1.5 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text placeholder-commander-subtext/40 focus:outline-none focus:border-commander-accent disabled:opacity-50"
              aria-label={title}
            />

            {error && (
              <div className="flex items-center gap-1.5 text-xs text-rose-400 bg-rose-950/40 p-2 rounded border border-rose-900/60">
                <AlertCircle className="w-4 h-4 shrink-0" />
                <span>{error}</span>
              </div>
            )}
          </div>

          <div className="flex items-center justify-end gap-2 px-4 py-3 bg-commander-darker border-t border-commander-border">
            <button
              type="button"
              onClick={onCancel}
              disabled={isLoading}
              className="px-3 py-1 text-xs rounded border border-commander-border hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isLoading || !value.trim()}
              className="px-3 py-1 text-xs rounded bg-commander-accent text-commander-dark font-medium hover:bg-commander-accent/90 transition-colors disabled:opacity-50 cursor-pointer"
            >
              {isLoading ? "Processing..." : confirmLabel}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
