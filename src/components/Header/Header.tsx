import React, { useState, useRef, useEffect } from "react";
import { FolderSync, Terminal, SlidersHorizontal, Server, Sun, Moon, Check } from "lucide-react";

interface HeaderProps {
  appName: string;
  appVersion: string;
  canOpenTerminal?: boolean;
  isDarkMode?: boolean;
  onToggleTheme?: () => void;
  onOpenSiteManager?: () => void;
  onOpenTerminal?: () => void;
  onOpenTerminalSettings?: () => void;
}

export const Header: React.FC<HeaderProps> = ({
  appName,
  appVersion,
  canOpenTerminal = false,
  isDarkMode = true,
  onToggleTheme,
  onOpenSiteManager,
  onOpenTerminal,
  onOpenTerminalSettings,
}) => {
  const [openMenu, setOpenMenu] = useState<"file" | "commands" | "options" | null>(null);
  const navRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!openMenu) return;
    const handleClickOutside = (event: MouseEvent) => {
      if (navRef.current && !navRef.current.contains(event.target as Node)) {
        setOpenMenu(null);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [openMenu]);

  return (
    <header className="flex flex-col bg-commander-darker border-b border-commander-border text-xs select-none relative z-30">
      <div className="flex items-center justify-between px-3 py-1.5 bg-commander-surface/70">
        <div className="flex items-center gap-2 font-semibold text-commander-accent">
          <FolderSync className="w-4 h-4" />
          <span>{appName}</span>
          <span className="text-[10px] text-commander-subtext px-1.5 py-0.5 rounded bg-commander-dark border border-commander-border">
            v{appVersion}
          </span>
        </div>
        <div className="text-[11px] text-commander-subtext">
          Commander Dual-Pane File Transfer Client
        </div>
      </div>
      <nav ref={navRef} className="flex items-center gap-4 px-3 py-1 text-commander-subtext">
        <div className="relative">
          <span
            onClick={() => setOpenMenu((prev) => (prev === "file" ? null : "file"))}
            className="cursor-pointer hover:text-commander-text transition-colors"
          >
            File
          </span>
          {openMenu === "file" && (
            <div
              role="menu"
              aria-label="File menu"
              className="absolute left-0 top-5 w-48 rounded border border-commander-border bg-commander-surface shadow-xl py-1 z-40"
            >
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setOpenMenu(null);
                  onOpenSiteManager?.();
                }}
                className="w-full flex items-center gap-2 px-3 py-1.5 text-left text-commander-text hover:bg-commander-selected transition-colors"
              >
                <Server className="w-3.5 h-3.5 text-commander-accent" />
                <span>Site Manager...</span>
              </button>
            </div>
          )}
        </div>

        <div className="relative">
          <span
            onClick={() => setOpenMenu((prev) => (prev === "commands" ? null : "commands"))}
            className="cursor-pointer hover:text-commander-text transition-colors"
          >
            Commands
          </span>
          {openMenu === "commands" && (
            <div
              role="menu"
              aria-label="Commands menu"
              className="absolute left-0 top-5 w-52 rounded border border-commander-border bg-commander-surface shadow-xl py-1 z-40"
            >
              <button
                type="button"
                role="menuitem"
                disabled={!canOpenTerminal}
                onClick={() => {
                  setOpenMenu(null);
                  onOpenTerminal?.();
                }}
                className={`w-full flex items-center gap-2 px-3 py-1.5 text-left transition-colors ${
                  canOpenTerminal
                    ? "text-commander-text hover:bg-commander-selected cursor-pointer"
                    : "text-commander-subtext/40 cursor-not-allowed"
                }`}
              >
                <Terminal className="w-3.5 h-3.5 text-commander-accent" />
                <span>Open in SSH Terminal</span>
              </button>
            </div>
          )}
        </div>

        <div className="relative">
          <span
            onClick={() => setOpenMenu((prev) => (prev === "options" ? null : "options"))}
            className="cursor-pointer hover:text-commander-text transition-colors"
          >
            Options
          </span>
          {openMenu === "options" && (
            <div
              role="menu"
              aria-label="Options menu"
              className="absolute left-0 top-5 w-52 rounded border border-commander-border bg-commander-surface shadow-xl py-1 z-40"
            >
              <div className="px-3 py-1 text-[10px] font-semibold text-commander-subtext uppercase tracking-wider">
                Appearance
              </div>

              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setOpenMenu(null);
                  if (!isDarkMode) onToggleTheme?.();
                }}
                className={`w-full flex items-center justify-between px-3 py-1.5 text-left text-commander-text hover:bg-commander-selected transition-colors ${
                  isDarkMode ? "font-semibold text-commander-accent" : ""
                }`}
                title="Switch to Dark Mode"
              >
                <div className="flex items-center gap-2">
                  <Moon className="w-3.5 h-3.5 text-commander-accent" />
                  <span>Dark Mode</span>
                </div>
                {isDarkMode && <Check className="w-3.5 h-3.5 text-commander-accent" />}
              </button>

              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setOpenMenu(null);
                  if (isDarkMode) onToggleTheme?.();
                }}
                className={`w-full flex items-center justify-between px-3 py-1.5 text-left text-commander-text hover:bg-commander-selected transition-colors ${
                  !isDarkMode ? "font-semibold text-commander-accent" : ""
                }`}
                title="Switch to Light Mode"
              >
                <div className="flex items-center gap-2">
                  <Sun className="w-3.5 h-3.5 text-amber-500" />
                  <span>Light Mode</span>
                </div>
                {!isDarkMode && <Check className="w-3.5 h-3.5 text-commander-accent" />}
              </button>

              <div className="my-1 border-t border-commander-border/60" />

              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setOpenMenu(null);
                  onOpenTerminalSettings?.();
                }}
                className="w-full flex items-center gap-2 px-3 py-1.5 text-left text-commander-text hover:bg-commander-selected transition-colors"
              >
                <SlidersHorizontal className="w-3.5 h-3.5 text-commander-accent" />
                <span>Terminal Settings...</span>
              </button>
            </div>
          )}
        </div>

        <span className="cursor-pointer hover:text-commander-text transition-colors">Help</span>
      </nav>
    </header>
  );
};
