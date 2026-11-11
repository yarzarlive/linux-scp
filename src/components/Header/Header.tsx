import React from "react";
import { FolderSync } from "lucide-react";

interface HeaderProps {
  appName: string;
  appVersion: string;
}

export const Header: React.FC<HeaderProps> = ({ appName, appVersion }) => {
  return (
    <header className="flex flex-col bg-commander-darker border-b border-commander-border text-xs select-none">
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
      <nav className="flex items-center gap-4 px-3 py-1 text-commander-subtext">
        <span className="cursor-pointer hover:text-commander-text transition-colors">File</span>
        <span className="cursor-pointer hover:text-commander-text transition-colors">Commands</span>
        <span className="cursor-pointer hover:text-commander-text transition-colors">Options</span>
        <span className="cursor-pointer hover:text-commander-text transition-colors">Help</span>
      </nav>
    </header>
  );
};
