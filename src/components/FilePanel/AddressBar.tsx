import React, { useState, useEffect, useRef } from "react";
import {
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Home,
  HardDrive,
  RefreshCw,
  Eye,
  EyeOff,
  Save,
  ChevronDown,
  Trash2,
} from "lucide-react";
import { IpcClient } from "../../services/ipc";

interface AddressBarProps {
  path: string;
  onNavigate: (targetPath: string) => void;
  paneType?: "local" | "remote";
  siteId?: string;
  scope?: string;
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
  paneType = "local",
  siteId,
  scope,
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
  const [savedPaths, setSavedPaths] = useState<string[]>([]);
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);
  const dropdownContainerRef = useRef<HTMLDivElement>(null);

  const effectiveScope =
    scope ?? (paneType === "remote" && siteId ? `remote:${siteId}` : (paneType ?? "local"));
  const localStorageKey = `linuxscp:saved_paths:${effectiveScope}`;

  useEffect(() => {
    setInputValue(path);
  }, [path]);

  // Load saved paths from backend preference repository, falling back to localStorage
  useEffect(() => {
    setIsDropdownOpen(false);
    let isMounted = true;
    const loadPaths = async () => {
      try {
        const paths = await IpcClient.getSavedPaths(effectiveScope);
        if (isMounted && Array.isArray(paths)) {
          setSavedPaths(paths);
          return;
        }
      } catch {
        // Fallback to localStorage if IPC is unavailable
      }
      try {
        if (typeof window !== "undefined" && window.localStorage) {
          const raw = window.localStorage.getItem(localStorageKey);
          if (raw) {
            const parsed = JSON.parse(raw);
            if (isMounted && Array.isArray(parsed)) {
              setSavedPaths(parsed);
              return;
            }
          }
        }
      } catch {
        // ignore storage parse errors
      }
      if (isMounted) {
        setSavedPaths([]);
      }
    };
    loadPaths();
    return () => {
      isMounted = false;
    };
  }, [effectiveScope, localStorageKey]);

  // Handle clicking outside the dropdown to close it
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownContainerRef.current &&
        !dropdownContainerRef.current.contains(event.target as Node)
      ) {
        setIsDropdownOpen(false);
      }
    };

    if (isDropdownOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isDropdownOpen]);

  const persistPaths = async (newPaths: string[]) => {
    setSavedPaths(newPaths);
    try {
      if (typeof window !== "undefined" && window.localStorage) {
        window.localStorage.setItem(localStorageKey, JSON.stringify(newPaths));
      }
    } catch {
      // ignore local storage errors
    }
    try {
      await IpcClient.saveSavedPaths(effectiveScope, newPaths);
    } catch {
      // ignore IPC errors
    }
  };

  const handleSaveCurrentPath = () => {
    const trimmed = path.trim();
    if (!trimmed) return;
    const updated = [trimmed, ...savedPaths.filter((p) => p !== trimmed)];
    persistPaths(updated);
  };

  const handleDeleteSavedPath = (targetPath: string) => {
    const updated = savedPaths.filter((p) => p !== targetPath);
    persistPaths(updated);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!disabled && inputValue.trim()) {
      setIsDropdownOpen(false);
      onNavigate(inputValue.trim());
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      if (isDropdownOpen) {
        setIsDropdownOpen(false);
        e.preventDefault();
      } else {
        setInputValue(path);
      }
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

      {/* Path input bar & Saved Paths */}
      <div className="relative flex-1 min-w-0" ref={dropdownContainerRef}>
        <form onSubmit={handleSubmit} className="relative flex items-center w-full">
          <input
            type="text"
            value={inputValue}
            disabled={disabled}
            onChange={(e) => setInputValue(e.target.value)}
            onKeyDown={handleKeyDown}
            aria-label="Path location"
            className="w-full pl-2 pr-14 py-0.5 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text placeholder-commander-subtext/50 focus:outline-none focus:border-commander-accent disabled:opacity-50 disabled:cursor-not-allowed"
            placeholder="Path..."
          />
          <div className="absolute right-1 flex items-center gap-0.5">
            <button
              type="button"
              onClick={handleSaveCurrentPath}
              disabled={disabled || !path.trim()}
              className="p-1 rounded text-commander-subtext hover:text-commander-accent hover:bg-commander-surface/80 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
              title="Save current path"
              aria-label="Save current path"
            >
              <Save className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={() => setIsDropdownOpen((prev) => !prev)}
              disabled={disabled}
              className={`p-1 rounded transition-colors ${
                isDropdownOpen
                  ? "bg-commander-selected text-commander-accent"
                  : "text-commander-subtext hover:text-white hover:bg-commander-surface/80"
              } disabled:opacity-40 disabled:cursor-not-allowed`}
              title="Saved paths"
              aria-label="Saved paths"
              aria-expanded={isDropdownOpen}
            >
              <ChevronDown className="w-3.5 h-3.5" />
            </button>
          </div>
        </form>

        {/* Saved Paths Dropdown Menu */}
        {isDropdownOpen && (
          <div
            className="absolute left-0 right-0 top-full mt-1 max-h-60 overflow-y-auto bg-commander-surface border border-commander-border rounded shadow-xl z-50 py-1"
            role="menu"
            aria-label="Saved paths menu"
          >
            {savedPaths.length === 0 ? (
              <div className="px-3 py-2 text-xs text-commander-subtext italic">
                No saved paths yet. Click the save icon to bookmark this path.
              </div>
            ) : (
              savedPaths.map((savedPath) => (
                <div
                  key={savedPath}
                  role="menuitem"
                  onClick={() => {
                    onNavigate(savedPath);
                    setIsDropdownOpen(false);
                  }}
                  className="flex items-center justify-between gap-2 px-3 py-1.5 text-xs font-mono text-commander-text hover:bg-commander-dark hover:text-white cursor-pointer group transition-colors"
                >
                  <span className="truncate flex-1" title={savedPath}>
                    {savedPath}
                  </span>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDeleteSavedPath(savedPath);
                    }}
                    className="p-0.5 rounded text-commander-subtext hover:text-red-400 hover:bg-commander-surface transition-colors shrink-0"
                    title={`Delete ${savedPath}`}
                    aria-label={`Delete ${savedPath}`}
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              ))
            )}
          </div>
        )}
      </div>

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
