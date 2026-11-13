import { useEffect, useState, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { Terminal, AlertCircle, X, SlidersHorizontal } from "lucide-react";
import { Header } from "../components/Header/Header";
import { StatusBar } from "../components/StatusBar/StatusBar";
import { ConnectionToolbar } from "../components/Toolbars/ConnectionToolbar";
import { LocalPane } from "../components/FilePanel/LocalPane";
import { RemotePane } from "../components/FilePanel/RemotePane";
import { TransferQueue } from "../components/TransferQueue/TransferQueue";
import { TransferConflictModal } from "../components/TransferQueue/TransferConflictModal";
import { HorizontalSplitter, VerticalSplitter } from "../components/Common/Splitters";
import { SiteManagerModal } from "../components/SiteManager/SiteManagerModal";
import { HostKeyPromptModal } from "../components/SiteManager/HostKeyPromptModal";
import { TerminalSettingsModal } from "../components/TerminalSettings/TerminalSettingsModal";
import { RemoteEditPanel } from "../components/RemoteEditPanel/RemoteEditPanel";
import { IpcClient } from "../services/ipc";
import { SystemInfo } from "../types/ipc";
import { ConnectionProfile } from "../types/connection";
import { RemoteEditSession, RemoteEditUploadMode } from "../types/desktop";
import { ConnectOutcome, SessionStateSnapshot } from "../types/session";
import {
  ConflictDecision,
  TransferConflictPrompt,
  TransferJob,
  TransferProgressEvent,
} from "../types/transfer";
import { InternalDragPayload } from "../utils/dragDrop";

type HostKeyChallenge = Extract<ConnectOutcome, { outcome: "HostKeyUnknown" | "HostKeyChanged" }>;

export function App() {
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [backendConnected, setBackendConnected] = useState<boolean>(false);
  const [pingLatencyMs, setPingLatencyMs] = useState<number | null>(null);
  const [activePane, setActivePane] = useState<"local" | "remote">("local");

  // Site Manager & Terminal Settings modal visibility
  const [siteManagerOpen, setSiteManagerOpen] = useState(false);
  const [terminalSettingsOpen, setTerminalSettingsOpen] = useState(false);
  const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null);
  const [terminalFeedback, setTerminalFeedback] = useState<{
    kind: "spawned" | "error";
    message: string;
  } | null>(null);

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

  // Current directory paths for transfers
  const [localCurrentPath, setLocalCurrentPath] = useState<string>("");
  const [remoteCurrentPath, setRemoteCurrentPath] = useState<string>("");
  const [localRefreshTrigger, setLocalRefreshTrigger] = useState<number>(0);
  const [remoteRefreshTrigger, setRemoteRefreshTrigger] = useState<number>(0);

  // Transfer queue & conflict state
  const [transferJobs, setTransferJobs] = useState<TransferJob[]>([]);
  const [conflictQueue, setConflictQueue] = useState<TransferConflictPrompt[]>([]);
  const [activeDrag, setActiveDrag] = useState<InternalDragPayload | null>(null);
  const [remoteEditSessions, setRemoteEditSessions] = useState<RemoteEditSession[]>([]);
  const [remoteEditUploadMode, setRemoteEditUploadMode] = useState<RemoteEditUploadMode>("auto");

  const upsertRemoteEditSession = useCallback((session: RemoteEditSession) => {
    let didAutoUpload = false;
    setRemoteEditSessions((prev) => {
      const idx = prev.findIndex((s) => s.id === session.id);
      if (idx >= 0) {
        if (session.upload_count > prev[idx].upload_count) {
          didAutoUpload = true;
        }
        const next = [...prev];
        next[idx] = session;
        return next;
      }
      return [...prev, session];
    });
    if (didAutoUpload) {
      setRemoteRefreshTrigger((prev) => prev + 1);
    }
  }, []);

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
        if (currentSession.session?.current_remote_path) {
          setRemoteCurrentPath(currentSession.session.current_remote_path);
        }
        if (currentSession.session?.profile_id) {
          setSelectedProfileId(currentSession.session.profile_id);
          lastConnectedProfileIdRef.current = currentSession.session.profile_id;
        } else {
          try {
            const profiles = await IpcClient.listConnectionProfiles();
            const firstSsh = profiles.find((p) => p.protocol === "SFTP" || p.protocol === "SCP");
            if (firstSsh) {
              setSelectedProfileId(firstSsh.id);
            }
          } catch {
            // Ignore profile pre-fetch errors on startup
          }
        }

        const existingTransfers = await IpcClient.listTransfers();
        setTransferJobs(existingTransfers);

        try {
          const existingEdits = await IpcClient.listRemoteEditSessions();
          setRemoteEditSessions(existingEdits);
          const mode = await IpcClient.getRemoteEditUploadMode();
          setRemoteEditUploadMode(mode);
        } catch {
          // Ignore if remote edit sessions cannot be fetched on startup
        }
      } catch (err) {
        console.error("IPC initialization error:", err);
        setBackendConnected(false);
      }
    }

    initShell();
  }, []);

  // Subscribe to real backend transfer events
  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];

    async function subscribe() {
      try {
        const u1 = await listen<TransferJob>("transfer://updated", (event) => {
          if (!active) return;
          const updated = event.payload;
          setTransferJobs((prev) => {
            const idx = prev.findIndex((j) => j.id === updated.id);
            if (idx >= 0) {
              const next = [...prev];
              next[idx] = updated;
              return next;
            }
            return [...prev, updated];
          });

          if (updated.state.status === "Completed") {
            if (updated.direction === "upload") {
              setRemoteRefreshTrigger((prev) => prev + 1);
            } else if (updated.direction === "download") {
              setLocalRefreshTrigger((prev) => prev + 1);
            }
          }
        });
        unlisteners.push(u1);

        const u2 = await listen<TransferProgressEvent>("transfer://progress", (event) => {
          if (!active) return;
          const p = event.payload;
          setTransferJobs((prev) =>
            prev.map((j) =>
              j.id === p.job_id
                ? {
                    ...j,
                    transferred_bytes: p.transferred_bytes,
                    total_bytes: p.total_bytes ?? j.total_bytes,
                    speed_bps: p.speed_bps,
                    eta_seconds: p.eta_seconds,
                    completed_files: p.completed_files,
                    total_files: p.total_files,
                    current_file: p.current_file,
                  }
                : j
            )
          );
        });
        unlisteners.push(u2);

        const u3 = await listen<TransferConflictPrompt>("transfer://conflict", (event) => {
          if (!active) return;
          setConflictQueue((prev) => [...prev, event.payload]);
        });
        unlisteners.push(u3);

        const u4 = await listen<RemoteEditSession>("remote-edit://updated", (event) => {
          if (!active) return;
          upsertRemoteEditSession(event.payload);
        });
        unlisteners.push(u4);

        const u5 = await listen<string>("remote-edit://removed", (event) => {
          if (!active) return;
          const removedId = event.payload;
          setRemoteEditSessions((prev) => prev.filter((s) => s.id !== removedId));
        });
        unlisteners.push(u5);
      } catch {
        // Non-Tauri unit test environment
      }
    }

    subscribe();
    return () => {
      active = false;
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [upsertRemoteEditSession]);

  // Scan tracked remote edit working copies immediately when user returns to LinuxSCP window
  useEffect(() => {
    const handleFocus = () => {
      void IpcClient.scanRemoteEditChanges()
        .then((sessions) => {
          setRemoteEditSessions(sessions);
        })
        .catch(() => {
          // Ignore focus scan errors in non-Tauri environments
        });
    };
    window.addEventListener("focus", handleFocus);
    return () => {
      window.removeEventListener("focus", handleFocus);
    };
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
    setSelectedProfileId(profile.id);

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
  const effectiveTerminalProfileId =
    sessionState.session?.profile_id || lastConnectedProfileIdRef.current || selectedProfileId;
  const canOpenTerminal = Boolean(effectiveTerminalProfileId);

  const handleOpenTerminal = useCallback(
    async (overrideProfileId?: string) => {
      const targetId = overrideProfileId || effectiveTerminalProfileId;
      if (!targetId) {
        setSiteManagerOpen(true);
        return;
      }

      setTerminalFeedback(null);
      try {
        const result = await IpcClient.launchSshTerminal(targetId);
        setTerminalFeedback({
          kind: "spawned",
          message: result.message,
        });
      } catch (err: unknown) {
        const message =
          typeof err === "object" && err !== null && "message" in err
            ? String((err as { message: unknown }).message)
            : String(err);
        setTerminalFeedback({
          kind: "error",
          message,
        });
      }
    },
    [effectiveTerminalProfileId]
  );

  const handleUpload = useCallback(
    async (localPaths: string[], targetRemoteDir?: string) => {
      const targetDir =
        targetRemoteDir || remoteCurrentPath || sessionState.session?.current_remote_path;
      if (!targetDir || localPaths.length === 0) return;
      try {
        const jobs = await IpcClient.enqueueUploads(localPaths, targetDir, "ask");
        setTransferJobs((prev) => {
          const existingIds = new Set(prev.map((j) => j.id));
          const added = jobs.filter((j) => !existingIds.has(j.id));
          return [...prev, ...added];
        });
        if (jobs.some((j) => j.state.status === "Completed")) {
          setRemoteRefreshTrigger((prev) => prev + 1);
        }
      } catch (err) {
        console.error("Upload enqueue error:", err);
      }
    },
    [remoteCurrentPath, sessionState.session?.current_remote_path]
  );

  const handleDownload = useCallback(
    async (remotePaths: string[], targetLocalDir?: string) => {
      const targetDir = targetLocalDir || localCurrentPath;
      if (!targetDir || remotePaths.length === 0) return;
      try {
        const jobs = await IpcClient.enqueueDownloads(remotePaths, targetDir, "ask");
        setTransferJobs((prev) => {
          const existingIds = new Set(prev.map((j) => j.id));
          const added = jobs.filter((j) => !existingIds.has(j.id));
          return [...prev, ...added];
        });
        if (jobs.some((j) => j.state.status === "Completed")) {
          setLocalRefreshTrigger((prev) => prev + 1);
        }
      } catch (err) {
        console.error("Download enqueue error:", err);
      }
    },
    [localCurrentPath]
  );

  const handleCancelTransfer = useCallback(async (jobId: string) => {
    try {
      await IpcClient.cancelTransfer(jobId);
    } catch (err) {
      console.error("Cancel transfer error:", err);
    }
  }, []);

  const handleRetryTransfer = useCallback(async (jobId: string) => {
    try {
      const updated = await IpcClient.retryTransfer(jobId);
      setTransferJobs((prev) => prev.map((j) => (j.id === updated.id ? updated : j)));
    } catch (err) {
      console.error("Retry transfer error:", err);
    }
  }, []);

  const handleClearFinishedTransfers = useCallback(async () => {
    try {
      const remaining = await IpcClient.clearFinishedTransfers();
      setTransferJobs(remaining);
    } catch (err) {
      console.error("Clear finished transfers error:", err);
    }
  }, []);

  const handleResolveConflict = useCallback(
    async (conflictId: string, decision: ConflictDecision, applyToAll: boolean) => {
      setConflictQueue((prev) => prev.filter((c) => c.conflict_id !== conflictId));
      try {
        await IpcClient.resolveTransferConflict(conflictId, decision, applyToAll);
      } catch (err) {
        console.error("Resolve conflict error:", err);
      }
    },
    []
  );

  const handleUploadRemoteEdit = useCallback(
    async (sessionId: string, overwriteRemoteChanges: boolean) => {
      const outcome = await IpcClient.uploadRemoteEditSession(sessionId, overwriteRemoteChanges);
      upsertRemoteEditSession(outcome.session);
      if (outcome.outcome === "Uploaded") {
        setRemoteRefreshTrigger((prev) => prev + 1);
      }
    },
    [upsertRemoteEditSession]
  );

  const handleKeepLocalRemoteEdit = useCallback(
    async (sessionId: string) => {
      const updated = await IpcClient.keepLocalRemoteEditChanges(sessionId);
      upsertRemoteEditSession(updated);
    },
    [upsertRemoteEditSession]
  );

  const handleSaveRemoteEditCopy = useCallback(
    async (sessionId: string, destinationDir: string): Promise<string | null> => {
      const targetDir = destinationDir || localCurrentPath || "/tmp";
      const savedPath = await IpcClient.saveRemoteEditLocalCopy(sessionId, targetDir);
      setLocalRefreshTrigger((prev) => prev + 1);
      return savedPath;
    },
    [localCurrentPath]
  );

  const handleDiscardRemoteEdit = useCallback(
    async (sessionId: string, discardLocalChanges: boolean) => {
      await IpcClient.discardRemoteEditSession(sessionId, discardLocalChanges);
      setRemoteEditSessions((prev) => prev.filter((s) => s.id !== sessionId));
    },
    []
  );

  const handleReloadRemoteEditFromRemote = useCallback(
    async (sessionId: string) => {
      const updated = await IpcClient.reloadRemoteEditFromRemote(sessionId, true);
      upsertRemoteEditSession(updated);
    },
    [upsertRemoteEditSession]
  );

  const handleChangeRemoteEditUploadMode = useCallback(async (mode: RemoteEditUploadMode) => {
    const saved = await IpcClient.setRemoteEditUploadMode(mode);
    setRemoteEditUploadMode(saved);
    if (saved === "auto") {
      const sessions = await IpcClient.scanRemoteEditChanges();
      setRemoteEditSessions(sessions);
    }
  }, []);

  const handleScanRemoteEdits = useCallback(async () => {
    const sessions = await IpcClient.scanRemoteEditChanges();
    setRemoteEditSessions(sessions);
  }, []);

  return (
    <div
      onDragOver={(e) => {
        e.preventDefault();
        if (e.dataTransfer) {
          e.dataTransfer.dropEffect = "none";
        }
      }}
      onDrop={(e) => {
        e.preventDefault();
        setActiveDrag(null);
      }}
      className="flex flex-col h-screen w-screen bg-commander-dark text-commander-text font-sans overflow-hidden"
    >
      {/* 1. Main Menu Area */}
      <Header
        appName="LinuxSCP"
        appVersion={systemInfo?.app_version ?? "0.1.0"}
        canOpenTerminal={canOpenTerminal}
        onOpenSiteManager={() => setSiteManagerOpen(true)}
        onOpenTerminal={() => handleOpenTerminal()}
        onOpenTerminalSettings={() => setTerminalSettingsOpen(true)}
      />

      {/* 2. Connection / Profile Toolbar */}
      <ConnectionToolbar
        isConnected={isConnected}
        isConnecting={isConnecting}
        canReconnect={canReconnect}
        canOpenTerminal={canOpenTerminal}
        activeSession={sessionState.session}
        hostKeyVerified={true}
        onNewSession={() => setSiteManagerOpen(true)}
        onSiteManager={() => setSiteManagerOpen(true)}
        onDisconnect={handleDisconnect}
        onReconnect={canReconnect ? handleReconnect : undefined}
        onOpenTerminal={() => handleOpenTerminal()}
        onOpenTerminalSettings={() => setTerminalSettingsOpen(true)}
      />

      {/* Optional Terminal Launch / Error Feedback Banner */}
      {terminalFeedback && (
        <div
          role={terminalFeedback.kind === "error" ? "alert" : "status"}
          className={`flex items-center justify-between px-3 py-1.5 border-b text-xs select-none ${
            terminalFeedback.kind === "error"
              ? "bg-rose-950/90 border-rose-800 text-rose-200"
              : "bg-sky-950/80 border-sky-800/70 text-sky-200"
          }`}
        >
          <div className="flex items-center gap-2 min-w-0 pr-2">
            {terminalFeedback.kind === "error" ? (
              <AlertCircle className="w-3.5 h-3.5 text-rose-400 shrink-0" />
            ) : (
              <Terminal className="w-3.5 h-3.5 text-sky-400 shrink-0" />
            )}
            <span className="truncate">{terminalFeedback.message}</span>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {terminalFeedback.kind === "error" && (
              <button
                type="button"
                onClick={() => setTerminalSettingsOpen(true)}
                className="flex items-center gap-1 px-2 py-0.5 rounded bg-rose-900/60 hover:bg-rose-900 border border-rose-700 text-rose-100 text-[11px] transition-colors"
              >
                <SlidersHorizontal className="w-3 h-3" />
                <span>Terminal Settings</span>
              </button>
            )}
            <button
              type="button"
              onClick={() => setTerminalFeedback(null)}
              className="p-0.5 rounded hover:bg-black/30 text-current opacity-80 hover:opacity-100"
              title="Dismiss terminal notice"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      )}

      {/* 2b. Tracked Remote File Editing Sessions Banner */}
      <RemoteEditPanel
        sessions={remoteEditSessions}
        isConnected={isConnected}
        localCurrentDir={localCurrentPath}
        uploadMode={remoteEditUploadMode}
        onChangeUploadMode={handleChangeRemoteEditUploadMode}
        onUploadSession={handleUploadRemoteEdit}
        onKeepLocalChanges={handleKeepLocalRemoteEdit}
        onSaveLocalCopy={handleSaveRemoteEditCopy}
        onDiscardSession={handleDiscardRemoteEdit}
        onReloadFromRemote={handleReloadRemoteEditFromRemote}
        onDownloadNewerRemote={async (remotePath, destDir) => {
          await handleDownload([remotePath], destDir || localCurrentPath);
        }}
        onScanNow={handleScanRemoteEdits}
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
              onDirectoryChange={setLocalCurrentPath}
              canUpload={isConnected}
              onUpload={handleUpload}
              onDropDownload={handleDownload}
              activeDrag={activeDrag}
              onInternalDragStart={setActiveDrag}
              onInternalDragEnd={() => setActiveDrag(null)}
              isActivePane={activePane === "local"}
              onFocusPane={() => setActivePane("local")}
              refreshTrigger={localRefreshTrigger}
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
              onDirectoryChange={setRemoteCurrentPath}
              onDownload={handleDownload}
              onDropUpload={handleUpload}
              activeDrag={activeDrag}
              onInternalDragStart={setActiveDrag}
              onInternalDragEnd={() => setActiveDrag(null)}
              refreshTrigger={remoteRefreshTrigger}
              onRemoteEditOpened={upsertRemoteEditSession}
            />
          </div>
        </div>

        {/* Resizable Vertical Divider */}
        {!queueCollapsed && <VerticalSplitter onResize={handleVerticalResize} />}

        {/* 4. Transfer Queue Area */}
        <TransferQueue
          jobs={transferJobs}
          height={queueHeight}
          isCollapsed={queueCollapsed}
          onToggleCollapse={() => setQueueCollapsed((prev) => !prev)}
          onCancelJob={handleCancelTransfer}
          onRetryJob={handleRetryTransfer}
          onClearFinished={handleClearFinishedTransfers}
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
        onSelectProfileId={setSelectedProfileId}
        onSelectAndConnect={(profile) => {
          handleConnectProfile(profile);
        }}
        onOpenTerminal={(profile) => {
          setSelectedProfileId(profile.id);
          setSiteManagerOpen(false);
          handleOpenTerminal(profile.id);
        }}
      />

      {/* 7. External SSH Terminal Settings Modal */}
      <TerminalSettingsModal
        isOpen={terminalSettingsOpen}
        onClose={() => setTerminalSettingsOpen(false)}
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

      {/* 8. Transfer Name Conflict Resolution Modal */}
      <TransferConflictModal
        conflict={conflictQueue[0] ?? null}
        onResolve={handleResolveConflict}
      />
    </div>
  );
}
