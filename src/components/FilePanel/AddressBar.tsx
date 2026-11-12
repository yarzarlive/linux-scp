import React, { useState, useEffect } from "react";
import {
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Home,
  HardDrive,
  RefreshCw,
  Eye,
  EyeOff,
} from "lucide-react";

interface AddressBarProps {
  path: string;
  onNavigate: (targetPath: string) => void;
  onBack?: () => void;
  canGoBack?: boolean;
  onForward?: () => void;
  canGoForward?: boolean;
  onUp?: () => void;
  canGoUp?: boolean;
  onHome?: () => void;
  onRoot?: () => void;
  onRefresh?: () => void;
  showHidden?: boolean;
  onToggleHidden?: () => void;
  disabled?: boolean;
  isLoading?: boolean;
}

export const AddressBar: React.FC<AddressBarProps> = ({
  path,
  onNavigate,
  onBack,
  canGoBack = false,
  onForward,
  canGoForward = false,
  onUp,
  canGoUp = false,
  onHome,
  onRoot,
  onRefresh,
  showHidden = false,
  onToggleHidden,
  disabled = false,
  isLoading = false,
}) => {
  const [inputValue, setInputValue] = useState(path);

  useEffect(() => {
    setInputValue(path);
  }, [path]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!disabled && inputValue.trim()) {
      onNavigate(inputValue.trim());
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      setInputValue(path);
    }
  };

  return (
    <div className="flex items-center gap-1 p-1 bg-commander-surface border-b border-commander-border text-xs select-none">
      {/* Navigation history controls */}
      <div className="flex items-center gap-0.5">
        <button
          type="button"
          onClick={onBack}
          disabled={disabled || !canGoBack}
          className={`p-1 rounded transition-colors ${
            canGoBack && !disabled
              ? "hover:bg-commander-dark text-commander-text hover:text-white"
              : "text-commander-subtext/40 cursor-not-allowed"
          }`}
          title="Back (Alt+Left)"
        >
          <ArrowLeft className="w-3.5 h-3.5" />
        </button>

        <button
          type="button"
          onClick={onForward}
          disabled={disabled || !canGoForward}
          className={`p-1 rounded transition-colors ${
            canGoForward && !disabled
              ? "hover:bg-commander-dark text-commander-text hover:text-white"
              : "text-commander-subtext/40 cursor-not-allowed"
          }`}
          title="Forward (Alt+Right)"
        >
          <ArrowRight className="w-3.5 h-3.5" />
        </button>

        <button
          type="button"
          onClick={onUp}
          disabled={disabled || !canGoUp}
          className={`p-1 rounded transition-colors ${
            canGoUp && !disabled
              ? "hover:bg-commander-dark text-commander-text hover:text-white"
              : "text-commander-subtext/40 cursor-not-allowed"
          }`}
          title="Parent Directory (..)"
        >
          <ArrowUp className="w-3.5 h-3.5" />
        </button>
      </div>

      <div className="h-4 w-px bg-commander-border/60 mx-0.5" />

      {/* Quick shortcuts */}
      <div className="flex items-center gap-0.5">
        <button
          type="button"
          onClick={onHome}
          disabled={disabled}
          className={`p-1 rounded transition-colors ${
            !disabled
              ? "hover:bg-commander-dark text-commander-text hover:text-white"
              : "text-commander-subtext/40 cursor-not-allowed"
          }`}
          title="Home Directory (~)"
        >
          <Home className="w-3.5 h-3.5" />
        </button>

        <button
          type="button"
          onClick={onRoot}
          disabled={disabled}
          className={`p-1 rounded transition-colors ${
            !disabled
              ? "hover:bg-commander-dark text-commander-text hover:text-white"
              : "text-commander-subtext/40 cursor-not-allowed"
          }`}
          title="Root Directory (/)"
        >
          <HardDrive className="w-3.5 h-3.5" />
        </button>
      </div>

      <div className="h-4 w-px bg-commander-border/60 mx-0.5" />

      {/* Path input bar */}
      <form onSubmit={handleSubmit} className="flex-1 min-w-0">
        <input
          type="text"
          value={inputValue}
          disabled={disabled}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={handleKeyDown}
          aria-label="Path location"
          className="w-full px-2 py-0.5 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text placeholder-commander-subtext/50 focus:outline-none focus:border-commander-accent disabled:opacity-50 disabled:cursor-not-allowed"
          placeholder="Path..."
        />
      </form>

      {/* Refresh & View buttons */}
      <div className="flex items-center gap-0.5">
        <button
          type="button"
          onClick={onRefresh}
          disabled={disabled || isLoading}
          className={`p-1 rounded transition-colors ${
            !disabled
              ? "hover:bg-commander-dark text-commander-text hover:text-white"
              : "text-commander-subtext/40 cursor-not-allowed"
          }`}
          title="Refresh (F5)"
        >
          <RefreshCw
            className={`w-3.5 h-3.5 ${isLoading ? "animate-spin text-commander-accent" : ""}`}
          />
        </button>

        {onToggleHidden && (
          <button
            type="button"
            onClick={onToggleHidden}
            disabled={disabled}
            className={`p-1 rounded transition-colors ${
              showHidden
                ? "bg-commander-selected text-commander-accent"
                : "hover:bg-commander-dark text-commander-subtext hover:text-white"
            } ${disabled ? "opacity-50 cursor-not-allowed" : ""}`}
            title={showHidden ? "Hide Hidden Files (Ctrl+H)" : "Show Hidden Files (Ctrl+H)"}
          >
            {showHidden ? <Eye className="w-3.5 h-3.5" /> : <EyeOff className="w-3.5 h-3.5" />}
          </button>
        )}
      </div>
    </div>
  );
};
