import { useEffect, useState, useCallback } from "react";
import { Header } from "../components/Header/Header";
import { StatusBar } from "../components/StatusBar/StatusBar";
import { ConnectionToolbar } from "../components/Toolbars/ConnectionToolbar";
import { LocalPane } from "../components/FilePanel/LocalPane";
import { RemotePane } from "../components/FilePanel/RemotePane";
import { TransferQueue } from "../components/TransferQueue/TransferQueue";
import { HorizontalSplitter, VerticalSplitter } from "../components/Common/Splitters";
import { SiteManagerModal } from "../components/SiteManager/SiteManagerModal";
import { IpcClient } from "../services/ipc";
import { SystemInfo } from "../types/ipc";

export function App() {
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [backendConnected, setBackendConnected] = useState<boolean>(false);
  const [pingLatencyMs, setPingLatencyMs] = useState<number | null>(null);
  const [activePane, setActivePane] = useState<"local" | "remote">("local");

  // Site Manager modal visibility
  const [siteManagerOpen, setSiteManagerOpen] = useState(false);

  // Pane width split percentage (default 50%)
  const [leftPanePercent, setLeftPanePercent] = useState<number>(50);

  // Transfer queue height in px (default 112px = h-28)
  const [queueHeight, setQueueHeight] = useState<number>(112);
  const [queueCollapsed, setQueueCollapsed] = useState<boolean>(false);

  useEffect(() => {
    async function initShell() {
      const start = performance.now();
      try {
        const ping = await IpcClient.pingBackend("startup");
        const elapsed = Math.round(performance.now() - start);
        setPingLatencyMs(elapsed);
        setBackendConnected(ping.status.startsWith("ACK"));

        const info = await IpcClient.getSystemInfo();
        setSystemInfo(info);
      } catch (err) {
        console.error("IPC initialization error:", err);
        setBackendConnected(false);
      }
    }

    initShell();
  }, []);

  // Splitter handlers
  const handleHorizontalResize = useCallback((deltaX: number) => {
    setLeftPanePercent((prev) => {
      const windowWidth = window.innerWidth || 1024;
      const deltaPercent = (deltaX / windowWidth) * 100;
      const next = prev + deltaPercent;
      return Math.min(Math.max(next, 20), 80); // clamp between 20% and 80%
    });
  }, []);

  const handleVerticalResize = useCallback((deltaY: number) => {
    setQueueHeight((prev) => {
      const next = prev - deltaY; // dragging up increases height
      return Math.min(Math.max(next, 70), 400); // clamp between 70px and 400px
    });
  }, []);

  return (
    <div className="flex flex-col h-screen w-screen bg-commander-dark text-commander-text font-sans overflow-hidden">
      {/* 1. Main Menu Area */}
      <Header appName="LinuxSCP" appVersion={systemInfo?.app_version ?? "0.1.0"} />

      {/* 2. Connection / Profile Toolbar */}
      <ConnectionToolbar
        isConnected={false}
        hostKeyVerified={true}
        onNewSession={() => setSiteManagerOpen(true)}
        onSiteManager={() => setSiteManagerOpen(true)}
        onDisconnect={() => {}}
      />

      {/* 3. Commander Dual-Pane Working Area */}
      <main className="flex-1 flex flex-col p-1.5 gap-1.5 min-h-0 overflow-hidden">
        <div className="flex-1 flex min-h-0 overflow-hidden">
          {/* Left: Local Filesystem Pane */}
          <div
            style={{ width: `${leftPanePercent}%` }}
            className="h-full flex flex-col min-w-[280px]"
          >
            <LocalPane
              isActivePane={activePane === "local"}
              onFocusPane={() => setActivePane("local")}
            />
          </div>

          {/* Resizable Horizontal Divider */}
          <HorizontalSplitter onResize={handleHorizontalResize} />

          {/* Right: Remote Filesystem Pane (Disconnected State) */}
          <div
            style={{ width: `${100 - leftPanePercent}%` }}
            className="h-full flex flex-col min-w-[280px]"
          >
            <RemotePane
              isActivePane={activePane === "remote"}
              onFocusPane={() => setActivePane("remote")}
              onConnect={() => setSiteManagerOpen(true)}
            />
          </div>
        </div>

        {/* Resizable Vertical Divider */}
        {!queueCollapsed && <VerticalSplitter onResize={handleVerticalResize} />}

        {/* 4. Transfer Queue Area */}
        <TransferQueue
          height={queueHeight}
          isCollapsed={queueCollapsed}
          onToggleCollapse={() => setQueueCollapsed((prev) => !prev)}
        />
      </main>

      {/* 5. Status Bar */}
      <StatusBar
        systemInfo={systemInfo}
        backendConnected={backendConnected}
        pingLatencyMs={pingLatencyMs}
      />

      {/* 6. Site Manager Modal */}
      <SiteManagerModal
        isOpen={siteManagerOpen}
        onClose={() => setSiteManagerOpen(false)}
        onSelectAndConnect={(profile) => {
          console.log("Profile selected for connection:", profile.name);
          setSiteManagerOpen(false);
        }}
      />
    </div>
  );
}
