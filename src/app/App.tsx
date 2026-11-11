import { useEffect, useState } from "react";
import { Header } from "../components/Header/Header";
import { StatusBar } from "../components/StatusBar/StatusBar";
import { IpcClient } from "../services/ipc";
import { SystemInfo } from "../types/ipc";
import { Folder, Server, ArrowLeftRight } from "lucide-react";

export function App() {
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [backendConnected, setBackendConnected] = useState<boolean>(false);
  const [pingLatencyMs, setPingLatencyMs] = useState<number | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

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
        setErrorMessage(
          err && typeof err === "object" && "message" in err
            ? String(err.message)
            : "Failed to communicate with Tauri backend."
        );
      }
    }

    initShell();
  }, []);

  return (
    <div className="flex flex-col h-screen w-screen bg-commander-dark text-commander-text font-sans">
      <Header
        appName={systemInfo?.app_name ? "LinuxSCP" : "LinuxSCP"}
        appVersion={systemInfo?.app_version ?? "0.1.0"}
      />

      {/* Main Dual-Pane Window Scaffold */}
      <main className="flex-1 flex flex-col p-2 gap-2 overflow-hidden">
        {errorMessage && (
          <div className="p-2 bg-rose-950/80 border border-rose-800 text-rose-200 text-xs rounded">
            <strong>Initialization Error:</strong> {errorMessage}
          </div>
        )}

        <div className="flex-1 grid grid-cols-2 gap-2 min-h-0">
          {/* Local Pane Scaffold */}
          <section className="flex flex-col rounded border border-commander-border bg-commander-surface overflow-hidden">
            <div className="flex items-center justify-between px-3 py-1.5 bg-commander-darker border-b border-commander-border text-xs font-medium">
              <div className="flex items-center gap-1.5 text-commander-accent">
                <Folder className="w-3.5 h-3.5" />
                <span>Local Pane</span>
              </div>
              <span className="text-[11px] text-commander-subtext">Local Filesystem</span>
            </div>
            <div className="flex-1 flex flex-col items-center justify-center p-6 text-center text-commander-subtext">
              <Folder className="w-10 h-10 mb-2 opacity-40 text-commander-accent" />
              <p className="text-xs">Local filesystem view ready for Milestone 1 implementation.</p>
              <p className="text-[11px] opacity-60 mt-1">
                Independent navigation and five-column file table.
              </p>
            </div>
          </section>

          {/* Remote Pane Scaffold */}
          <section className="flex flex-col rounded border border-commander-border bg-commander-surface overflow-hidden">
            <div className="flex items-center justify-between px-3 py-1.5 bg-commander-darker border-b border-commander-border text-xs font-medium">
              <div className="flex items-center gap-1.5 text-commander-accent">
                <Server className="w-3.5 h-3.5" />
                <span>Remote Pane</span>
              </div>
              <span className="text-[11px] text-commander-subtext">Remote Server</span>
            </div>
            <div className="flex-1 flex flex-col items-center justify-center p-6 text-center text-commander-subtext">
              <Server className="w-10 h-10 mb-2 opacity-40 text-commander-accent" />
              <p className="text-xs">Remote filesystem view ready for SFTP session integration.</p>
              <p className="text-[11px] opacity-60 mt-1">
                Cross-pane drag-and-drop and directory synchronization.
              </p>
            </div>
          </section>
        </div>

        {/* Transfer Queue Scaffold */}
        <section className="h-28 rounded border border-commander-border bg-commander-surface flex flex-col overflow-hidden">
          <div className="flex items-center justify-between px-3 py-1 bg-commander-darker border-b border-commander-border text-xs text-commander-subtext">
            <div className="flex items-center gap-1.5 font-medium text-commander-text">
              <ArrowLeftRight className="w-3.5 h-3.5 text-commander-accent" />
              <span>Transfer Queue</span>
            </div>
            <span className="text-[10px]">0 tasks active</span>
          </div>
          <div className="flex-1 flex items-center justify-center text-xs text-commander-subtext opacity-50">
            Queue idle. No active background file transfers.
          </div>
        </section>
      </main>

      <StatusBar
        systemInfo={systemInfo}
        backendConnected={backendConnected}
        pingLatencyMs={pingLatencyMs}
      />
    </div>
  );
}

export default App;
