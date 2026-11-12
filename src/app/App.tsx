import { useEffect, useState, useCallback, useRef } from "react";
import { Header } from "../components/Header/Header";
import { StatusBar } from "../components/StatusBar/StatusBar";
import { ConnectionToolbar } from "../components/Toolbars/ConnectionToolbar";
import { LocalPane } from "../components/FilePanel/LocalPane";
import { RemotePane } from "../components/FilePanel/RemotePane";
import { TransferQueue } from "../components/TransferQueue/TransferQueue";
import { HorizontalSplitter, VerticalSplitter } from "../components/Common/Splitters";
import { SiteManagerModal } from "../components/SiteManager/SiteManagerModal";
import { HostKeyPromptModal } from "../components/SiteManager/HostKeyPromptModal";
import { IpcClient } from "../services/ipc";
import { SystemInfo } from "../types/ipc";
import { ConnectionProfile } from "../types/connection";
import { ConnectOutcome, SessionStateSnapshot } from "../types/session";

type HostKeyChallenge = Extract<ConnectOutcome, { outcome: "HostKeyUnknown" | "HostKeyChanged" }>;

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

  // SFTP Session lifecycle state
  const [sessionState, setSessionState] = useState<SessionStateSnapshot>({
    status: { status: "Disconnected" },
    session: null,
    trusted_host_key: null,
  });
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [hostKeyChallenge, setHostKeyChallenge] = useState<HostKeyChallenge | null>(null);
  const [isTrustingHostKey, setIsTrustingHostKey] = useState(false);
  const [hostKeyModalError, setHostKeyModalError] = useState<string | null>(null);
  const lastConnectedProfileIdRef = useRef<string | null>(null);

  // Initial startup probe
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

        const currentSession = await IpcClient.getSessionState();
        setSessionState(currentSession);
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

  // Connect to a profile
  const handleConnectProfile = useCallback(async (profile: ConnectionProfile, secret?: string) => {
    setConnectionError(null);
    setHostKeyChallenge(null);
    setHostKeyModalError(null);
    lastConnectedProfileIdRef.current = profile.id;

    setSessionState({
      status: { status: "Connecting" },
      session: null,
      trusted_host_key: null,
    });

    try {
      const outcome = await IpcClient.connectSftpSession(profile.id, secret);

      if (outcome.outcome === "Connected") {
        setSessionState({
          status: { status: "Connected" },
          session: outcome.session,
          trusted_host_key: outcome.trusted_host_key,
        });
        setSiteManagerOpen(false);
      } else if (outcome.outcome === "HostKeyUnknown" || outcome.outcome === "HostKeyChanged") {
        setHostKeyChallenge(outcome);
        setSessionState({
          status: { status: "Disconnected" },
          session: null,
          trusted_host_key: null,
        });
        setSiteManagerOpen(false);
      }
    } catch (err) {
      const message =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : String(err);
      setConnectionError(message);
      setSessionState({
        status: {
          status: "Failed",
          details: { error_message: message },
        },
        session: null,
        trusted_host_key: null,
      });
    }
  }, []);

  // Reconnect active or last session
  const handleReconnect = useCallback(async () => {
    setConnectionError(null);
    setHostKeyChallenge(null);
    setHostKeyModalError(null);

    setSessionState((prev) => ({
      ...prev,
      status: { status: "Reconnecting" },
    }));

    try {
      const outcome = await IpcClient.reconnectSftpSession();

      if (outcome.outcome === "Connected") {
        setSessionState({
          status: { status: "Connected" },
          session: outcome.session,
          trusted_host_key: outcome.trusted_host_key,
        });
      } else if (outcome.outcome === "HostKeyUnknown" || outcome.outcome === "HostKeyChanged") {
        setHostKeyChallenge(outcome);
        setSessionState({
          status: { status: "Disconnected" },
          session: null,
          trusted_host_key: null,
        });
      }
    } catch (err) {
      const message =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : String(err);
      setConnectionError(message);
      setSessionState({
        status: {
          status: "Failed",
          details: { error_message: message },
        },
        session: null,
        trusted_host_key: null,
      });
    }
  }, []);

  // Disconnect active session or cancel connecting
  const handleDisconnect = useCallback(async () => {
    try {
      const snapshot = await IpcClient.disconnectSftpSession();
      setSessionState(snapshot);
      setConnectionError(null);
      setHostKeyChallenge(null);
    } catch (err) {
      console.error("Disconnect error:", err);
    }
  }, []);

  // User approved an unknown host key
  const handleApproveUnknownKey = useCallback(
    async (challenge: Extract<ConnectOutcome, { outcome: "HostKeyUnknown" }>) => {
      setIsTrustingHostKey(true);
      setHostKeyModalError(null);
      try {
        await IpcClient.trustHostKey(
          challenge.host,
          challenge.port,
          challenge.algorithm,
          challenge.fingerprint_sha256,
          challenge.public_key_openssh
        );

        setHostKeyChallenge(null);

        // Retry connection with the profile that was pending
        const profileId = lastConnectedProfileIdRef.current;
        if (profileId) {
          setSessionState({
            status: { status: "Connecting" },
            session: null,
            trusted_host_key: null,
          });
          const outcome = await IpcClient.connectSftpSession(profileId);
          if (outcome.outcome === "Connected") {
            setSessionState({
              status: { status: "Connected" },
              session: outcome.session,
              trusted_host_key: outcome.trusted_host_key,
            });
          }
        }
      } catch (err) {
        const message =
          typeof err === "object" && err !== null && "message" in err
            ? String((err as { message: unknown }).message)
            : String(err);
        setHostKeyModalError(message);
      } finally {
        setIsTrustingHostKey(false);
      }
    },
    []
  );

  const isConnected = sessionState.status.status === "Connected" && Boolean(sessionState.session);
  const isConnecting =
    sessionState.status.status === "Connecting" || sessionState.status.status === "Reconnecting";
  const canReconnect = Boolean(lastConnectedProfileIdRef.current || isConnected);

  return (
    <div className="flex flex-col h-screen w-screen bg-commander-dark text-commander-text font-sans overflow-hidden">
      {/* 1. Main Menu Area */}
      <Header appName="LinuxSCP" appVersion={systemInfo?.app_version ?? "0.1.0"} />

      {/* 2. Connection / Profile Toolbar */}
      <ConnectionToolbar
        isConnected={isConnected}
        isConnecting={isConnecting}
        canReconnect={canReconnect}
        activeSession={sessionState.session}
        hostKeyVerified={true}
        onNewSession={() => setSiteManagerOpen(true)}
        onSiteManager={() => setSiteManagerOpen(true)}
        onDisconnect={handleDisconnect}
        onReconnect={canReconnect ? handleReconnect : undefined}
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

          {/* Right: Remote Filesystem Pane */}
          <div
            style={{ width: `${100 - leftPanePercent}%` }}
            className="h-full flex flex-col min-w-[280px]"
          >
            <RemotePane
              sessionState={sessionState}
              connectionError={connectionError}
              isActivePane={activePane === "remote"}
              onFocusPane={() => setActivePane("remote")}
              onConnect={() => setSiteManagerOpen(true)}
              onDisconnect={handleDisconnect}
              onReconnect={handleReconnect}
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
        activeSession={sessionState.session}
      />

      {/* 6. Site Manager Modal */}
      <SiteManagerModal
        isOpen={siteManagerOpen}
        onClose={() => setSiteManagerOpen(false)}
        onSelectAndConnect={(profile) => {
          handleConnectProfile(profile);
        }}
      />

      {/* 7. Strict SSH Host Key Verification Modal */}
      <HostKeyPromptModal
        challenge={hostKeyChallenge}
        isProcessing={isTrustingHostKey}
        error={hostKeyModalError}
        onApproveUnknownKey={handleApproveUnknownKey}
        onCancel={() => {
          setHostKeyChallenge(null);
          setHostKeyModalError(null);
        }}
      />
    </div>
  );
}
