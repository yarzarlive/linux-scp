import React, { useState, useEffect, useMemo, useCallback, useRef } from "react";
import {
  Server,
  Plus,
  Copy,
  Trash2,
  Lock,
  Unlock,
  Key,
  Shield,
  ShieldAlert,
  AlertCircle,
  Eye,
  EyeOff,
  X,
  Search,
  Terminal,
  Play,
  Edit3,
  Download,
  Upload,
  ChevronDown,
  Check,
  SlidersHorizontal,
} from "lucide-react";
import {
  AuthMethod,
  ConnectionProfile,
  ConnectionProfileSummary,
  ProtocolType,
} from "../../types/connection";
import { IpcClient } from "../../services/ipc";
import { ConfirmDialog } from "../Common/ConfirmDialog";

interface SiteManagerModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSelectAndConnect?: (profile: ConnectionProfile) => void;
  onOpenTerminal?: (profile: ConnectionProfile) => void;
  onSelectProfileId?: (profileId: string | null) => void;
  initialProfileId?: string;
}

export const SiteManagerModal: React.FC<SiteManagerModalProps> = ({
  isOpen,
  onClose,
  onSelectAndConnect,
  onOpenTerminal,
  onSelectProfileId,
  initialProfileId,
}) => {
  const [profiles, setProfiles] = useState<ConnectionProfileSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false);
  const [profileToDelete, setProfileToDelete] = useState<
    ConnectionProfileSummary | ConnectionProfile | null
  >(null);
  const [showPassword, setShowPassword] = useState(false);
  const [keyringAvailable, setKeyringAvailable] = useState<boolean>(true);

  // Context menu state for connection items in sidebar
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    profile: ConnectionProfileSummary;
  } | null>(null);
  const contextMenuRef = useRef<HTMLDivElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);

  // Form state for currently edited profile
  const [formProfile, setFormProfile] = useState<ConnectionProfile | null>(null);
  const [formSecret, setFormSecret] = useState<string>("");

  // Lock site state (persisted to localStorage)
  const [sitesLocked, setSitesLocked] = useState<boolean>(() => {
    if (typeof window !== "undefined" && window.localStorage) {
      try {
        return window.localStorage.getItem("linuxscp:site_manager_locked") === "true";
      } catch {
        return false;
      }
    }
    return false;
  });

  // Config dropdown menu state
  const [configMenuOpen, setConfigMenuOpen] = useState(false);
  const configMenuRef = useRef<HTMLDivElement>(null);

  // Export dialog state
  const [exportDialogOpen, setExportDialogOpen] = useState(false);
  const [exportPath, setExportPath] = useState("");
  const [isExporting, setIsExporting] = useState(false);

  // Import dialog state
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [importConfirmOpen, setImportConfirmOpen] = useState(false);
  const [pendingImportJson, setPendingImportJson] = useState("");
  const [pendingImportCount, setPendingImportCount] = useState(0);
  const [pendingImportFileName, setPendingImportFileName] = useState("");
  const [isImporting, setIsImporting] = useState(false);

  // Success / feedback banner state
  const [feedbackMessage, setFeedbackMessage] = useState<string | null>(null);

  // Load profiles list and probe keyring
  const loadProfiles = useCallback(
    async (selectIdAfterLoad?: string) => {
      setIsLoading(true);
      setErrorMessage(null);
      try {
        const list = await IpcClient.listConnectionProfiles();
        setProfiles(list);

        const targetId = selectIdAfterLoad || selectedId || (list.length > 0 ? list[0].id : null);
        if (targetId && list.some((p) => p.id === targetId)) {
          setSelectedId(targetId);
          onSelectProfileId?.(targetId);
          const full = await IpcClient.getConnectionProfile(targetId);
          setFormProfile(full);
          setFormSecret("");
        } else if (list.length > 0) {
          setSelectedId(list[0].id);
          onSelectProfileId?.(list[0].id);
          const full = await IpcClient.getConnectionProfile(list[0].id);
          setFormProfile(full);
          setFormSecret("");
        } else {
          setSelectedId(null);
          onSelectProfileId?.(null);
          setFormProfile(null);
        }

        // Probe keyring
        const status = await IpcClient.checkKeyringStatus();
        setKeyringAvailable(status);
      } catch (err: unknown) {
        console.error("Failed to load profiles:", err);
        const msg =
          err && typeof err === "object" && "message" in err
            ? String(err.message)
            : "Failed to load profiles";
        setErrorMessage(msg);
      } finally {
        setIsLoading(false);
      }
    },
    [selectedId, onSelectProfileId]
  );

  useEffect(() => {
    if (isOpen) {
      loadProfiles(initialProfileId);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, initialProfileId]);

  // Filtered profile list
  const filteredProfiles = useMemo(() => {
    if (!searchQuery.trim()) return profiles;
    const q = searchQuery.toLowerCase();
    return profiles.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.host.toLowerCase().includes(q) ||
        p.username.toLowerCase().includes(q)
    );
  }, [profiles, searchQuery]);

  // Handle selecting a profile from sidebar
  const handleSelectProfile = async (id: string) => {
    if (selectedId === id) return;
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const full = await IpcClient.getConnectionProfile(id);
      setSelectedId(id);
      onSelectProfileId?.(id);
      setFormProfile(full);
      setFormSecret("");
    } catch (err: unknown) {
      setErrorMessage(
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to load profile details"
      );
    } finally {
      setIsLoading(false);
    }
  };

  // Handle creating a new profile
  const handleNewProfile = () => {
    if (sitesLocked) return;
    const newId = `new-${Date.now()}`;
    const defaultProfile: ConnectionProfile = {
      id: newId,
      name: "New Connection",
      protocol: "SFTP",
      host: "127.0.0.1",
      port: 22,
      username: "user",
      auth_method: { type: "Password" },
      sudo_sftp: { enabled: false },
      transfer_settings: {
        preserve_timestamps: true,
        preserve_permissions: true,
        concurrency_limit: 2,
      },
      connection_settings: {
        timeout_seconds: 30,
        keepalive_interval_seconds: 15,
      },
      created_at_ms: Date.now(),
      updated_at_ms: Date.now(),
    };

    setFormProfile(defaultProfile);
    setFormSecret("");
    setSelectedId(newId);
    setErrorMessage(null);
  };

  // Handle duplicating current profile
  const handleDuplicate = async () => {
    if (sitesLocked || !selectedId || !formProfile) return;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const copyName = `${formProfile.name} (Copy)`;
      const dup = await IpcClient.duplicateConnectionProfile(selectedId, copyName);
      await loadProfiles(dup.id);
    } catch (err: unknown) {
      setErrorMessage(
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to duplicate profile"
      );
    } finally {
      setIsSaving(false);
    }
  };

  // Handle lock toggle
  const handleToggleLock = () => {
    setSitesLocked((prev) => {
      const next = !prev;
      if (typeof window !== "undefined" && window.localStorage) {
        try {
          window.localStorage.setItem("linuxscp:site_manager_locked", String(next));
        } catch {
          // ignore
        }
      }
      return next;
    });
  };

  // Close config dropdown menu on click outside
  useEffect(() => {
    if (!configMenuOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (configMenuRef.current && !configMenuRef.current.contains(e.target as Node)) {
        setConfigMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [configMenuOpen]);

  // Generate date-time formatted filename: linuxscp-sites-YYYYMMDD-HHmmss.json
  const generateExportFilename = () => {
    const d = new Date();
    const pad = (n: number) => String(n).padStart(2, "0");
    const yyyy = d.getFullYear();
    const mm = pad(d.getMonth() + 1);
    const dd = pad(d.getDate());
    const hh = pad(d.getHours());
    const min = pad(d.getMinutes());
    const ss = pad(d.getSeconds());
    return `linuxscp-sites-${yyyy}${mm}${dd}-${hh}${min}${ss}.json`;
  };

  // Open Export dialog
  const handleOpenExport = async () => {
    setConfigMenuOpen(false);
    setErrorMessage(null);
    setFeedbackMessage(null);
    let defaultDir = "";
    try {
      defaultDir = await IpcClient.getHomeDirectory();
    } catch {
      defaultDir = "~";
    }
    const filename = generateExportFilename();
    const fullDefault = defaultDir ? `${defaultDir.replace(/\/$/, "")}/${filename}` : filename;
    setExportPath(fullDefault);
    setExportDialogOpen(true);
  };

  // Execute Export
  const handleExecuteExport = async () => {
    if (!exportPath.trim()) return;
    setIsExporting(true);
    setErrorMessage(null);
    try {
      const result = await IpcClient.exportConnectionProfiles(exportPath.trim());
      // Also trigger browser download if available
      try {
        const blob = new Blob([result.json_content], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = exportPath.trim().split("/").pop() || generateExportFilename();
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      } catch {
        // ignore in headless test
      }
      setFeedbackMessage(
        `Successfully exported ${result.count} site configuration(s) to ${result.file_path || exportPath.trim()}`
      );
      setExportDialogOpen(false);
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to export site configurations";
      setErrorMessage(msg);
    } finally {
      setIsExporting(false);
    }
  };

  // Trigger file selection for Import
  const handleTriggerImportFile = () => {
    setConfigMenuOpen(false);
    setErrorMessage(null);
    setFeedbackMessage(null);
    if (fileInputRef.current) {
      fileInputRef.current.value = "";
      fileInputRef.current.click();
    }
  };

  // Handle selected Import file
  const handleImportFileSelected = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = () => {
      try {
        const content = String(reader.result || "");
        const parsed = JSON.parse(content);
        const count = Array.isArray(parsed)
          ? parsed.length
          : Array.isArray(parsed?.profiles)
            ? parsed.profiles.length
            : 0;

        if (count === 0) {
          setErrorMessage("The selected file contains no valid site configurations to import.");
          return;
        }

        setPendingImportJson(content);
        setPendingImportCount(count);
        setPendingImportFileName(file.name);
        setImportConfirmOpen(true);
      } catch {
        setErrorMessage("Invalid JSON file: unable to parse site configurations.");
      }
    };
    reader.onerror = () => {
      setErrorMessage("Failed to read the selected file.");
    };
    reader.readAsText(file);
  };

  // Execute Import (merge or replace)
  const handleExecuteImport = async (mode: "merge" | "replace") => {
    if (!pendingImportJson) return;
    setIsImporting(true);
    setErrorMessage(null);
    try {
      const result = await IpcClient.importConnectionProfiles(pendingImportJson, mode);
      setImportConfirmOpen(false);
      setPendingImportJson("");
      await loadProfiles();
      setFeedbackMessage(
        `Successfully imported ${result.imported_count} site(s) (${mode === "merge" ? "Merged" : "Replaced"}). Total sites: ${result.total_count}`
      );
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to import site configurations";
      setErrorMessage(msg);
    } finally {
      setIsImporting(false);
    }
  };

  // Dismiss context menu on click outside or Escape
  useEffect(() => {
    if (!contextMenu) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) {
        setContextMenu(null);
      }
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setContextMenu(null);
      }
    };

    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [contextMenu]);

  // Handle direct connection (on double-click or context menu "Connect")
  const handleDirectConnect = async (profileId: string) => {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const profile =
        formProfile && formProfile.id === profileId
          ? formProfile
          : await IpcClient.getConnectionProfile(profileId);
      if (onSelectAndConnect) {
        onSelectAndConnect(profile);
      } else {
        await IpcClient.connectSftpSession(profile.id);
      }
      onClose();
    } catch (err: unknown) {
      setErrorMessage(
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to connect to profile"
      );
    } finally {
      setIsLoading(false);
    }
  };

  // Open context menu on right click on a connection
  const handleContextMenu = (e: React.MouseEvent, p: ConnectionProfileSummary) => {
    e.preventDefault();
    e.stopPropagation();
    if (selectedId !== p.id) {
      handleSelectProfile(p.id);
    }
    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      profile: p,
    });
  };

  // Prompt delete confirmation
  const handlePromptDelete = (profile?: ConnectionProfileSummary | ConnectionProfile) => {
    if (sitesLocked) return;
    const target = profile || formProfile;
    if (!target) return;
    setProfileToDelete(target);
    setDeleteConfirmOpen(true);
  };

  // Handle delete confirmation
  const handleDeleteConfirm = async () => {
    const targetId = profileToDelete?.id || selectedId;
    if (!targetId) return;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      await IpcClient.deleteConnectionProfile(targetId);
      setDeleteConfirmOpen(false);
      setProfileToDelete(null);
      await loadProfiles();
    } catch (err: unknown) {
      setErrorMessage(
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to delete profile"
      );
    } finally {
      setIsSaving(false);
    }
  };

  // Protocol change auto-updates default port if untouched
  const handleProtocolChange = (nextProtocol: ProtocolType) => {
    if (!formProfile) return;
    const oldDefault =
      formProfile.protocol === "SFTP" || formProfile.protocol === "SCP"
        ? 22
        : formProfile.protocol === "FTPS"
          ? 990
          : 21;
    const newDefault =
      nextProtocol === "SFTP" || nextProtocol === "SCP" ? 22 : nextProtocol === "FTPS" ? 990 : 21;

    const nextPort = formProfile.port === oldDefault ? newDefault : formProfile.port;

    // Sudo-SFTP is only valid on SFTP
    const nextSudo =
      nextProtocol === "SFTP"
        ? formProfile.sudo_sftp
        : { ...formProfile.sudo_sftp, enabled: false };

    setFormProfile({
      ...formProfile,
      protocol: nextProtocol,
      port: nextPort,
      sudo_sftp: nextSudo,
    });
  };

  // Auth method change
  const handleAuthMethodTypeChange = (type: string) => {
    if (!formProfile) return;
    let nextAuth: AuthMethod;
    if (type === "Password") {
      nextAuth = { type: "Password" };
    } else if (type === "PublicKey") {
      nextAuth = {
        type: "PublicKey",
        details: { private_key_path: "~/.ssh/id_rsa", requires_passphrase: false },
      };
    } else if (type === "Agent") {
      nextAuth = { type: "Agent" };
    } else {
      nextAuth = { type: "Interactive" };
    }

    setFormProfile({
      ...formProfile,
      auth_method: nextAuth,
    });
  };

  // Save profile handler
  const handleSave = async (action: "save" | "connect" | "terminal" = "save") => {
    if (!formProfile) return;

    if (sitesLocked) {
      if (action === "connect") {
        onSelectAndConnect?.(formProfile);
        onClose();
        return;
      }
      if (action === "terminal") {
        onOpenTerminal?.(formProfile);
        return;
      }
      setErrorMessage("Site configurations are locked. Unlock sites to make changes.");
      return;
    }

    setIsSaving(true);
    setErrorMessage(null);

    // Frontend validation
    if (!formProfile.name.trim()) {
      setErrorMessage("Profile name cannot be empty");
      setIsSaving(false);
      return;
    }
    if (!formProfile.host.trim()) {
      setErrorMessage("Host address cannot be empty");
      setIsSaving(false);
      return;
    }
    if (formProfile.host.includes("://") || formProfile.host.includes(" ")) {
      setErrorMessage("Host must not contain URL schemes or whitespace");
      setIsSaving(false);
      return;
    }
    if (formProfile.port < 1 || formProfile.port > 65535) {
      setErrorMessage("Port must be between 1 and 65535");
      setIsSaving(false);
      return;
    }
    if (!formProfile.username.trim()) {
      setErrorMessage("Username cannot be empty");
      setIsSaving(false);
      return;
    }

    try {
      const saved = await IpcClient.saveConnectionProfile(
        formProfile,
        formSecret.trim() ? formSecret.trim() : undefined
      );

      await loadProfiles(saved.id);

      if (action === "connect") {
        onSelectAndConnect?.(saved);
        onClose();
      } else if (action === "terminal") {
        if (onOpenTerminal) {
          onOpenTerminal(saved);
        } else {
          await IpcClient.launchSshTerminal(saved.id);
        }
      }
    } catch (err: unknown) {
      setErrorMessage(
        err && typeof err === "object" && "message" in err
          ? String(err.message)
          : "Failed to save profile"
      );
    } finally {
      setIsSaving(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 select-none"
      role="dialog"
      aria-modal="true"
      aria-labelledby="site-manager-title"
    >
      <div className="w-full max-w-4xl h-[620px] rounded-lg border border-commander-border bg-commander-surface shadow-2xl flex flex-col overflow-hidden animate-in fade-in zoom-in-95 duration-100">
        {/* Title Bar */}
        <div className="flex items-center justify-between px-4 py-2.5 bg-commander-darker border-b border-commander-border">
          <div className="flex items-center gap-2">
            <Server className="w-4 h-4 text-commander-accent" />
            <h2 id="site-manager-title" className="text-sm font-semibold text-commander-text">
              Site Manager — Connection Profiles
            </h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
            title="Close dialog"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Main 2-Column Area */}
        <div className="flex-1 flex min-h-0 overflow-hidden">
          {/* Left Sidebar: Profile List */}
          <div className="w-72 border-r border-commander-border bg-commander-darker/60 flex flex-col min-h-0">
            {/* Search & Actions */}
            <div className="p-2 border-b border-commander-border space-y-2">
              <div className="relative">
                <Search className="w-3.5 h-3.5 absolute left-2 top-2 text-commander-subtext/60" />
                <input
                  type="text"
                  placeholder="Filter profiles..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full pl-7 pr-2 py-1 text-xs bg-commander-dark border border-commander-border rounded text-commander-text placeholder-commander-subtext/40 focus:outline-none focus:border-commander-accent"
                />
              </div>

              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={handleNewProfile}
                  disabled={sitesLocked}
                  className="flex-1 flex items-center justify-center gap-1 px-2 py-1 rounded bg-commander-accent/20 hover:bg-commander-accent/30 text-commander-accent text-xs font-medium transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                  title={sitesLocked ? "Site configurations are locked against edits" : "Create new profile"}
                >
                  <Plus className="w-3.5 h-3.5" />
                  <span>New</span>
                </button>

                <button
                  type="button"
                  onClick={handleDuplicate}
                  disabled={sitesLocked || !selectedId || !formProfile}
                  className="flex items-center justify-center p-1 rounded border border-commander-border hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                  title={sitesLocked ? "Site configurations are locked against edits" : "Duplicate selected profile"}
                >
                  <Copy className="w-3.5 h-3.5" />
                </button>

                <button
                  type="button"
                  onClick={() => handlePromptDelete()}
                  disabled={sitesLocked || !selectedId || !formProfile}
                  className="flex items-center justify-center p-1 rounded border border-rose-900/60 hover:bg-rose-950/40 text-rose-300 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                  title={sitesLocked ? "Site configurations are locked against edits" : "Delete selected profile"}
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            {/* Profile List Items */}
            <div className="flex-1 overflow-y-auto p-1 space-y-0.5">
              {isLoading ? (
                <div className="p-4 text-center text-xs text-commander-subtext/50">
                  Loading profiles...
                </div>
              ) : filteredProfiles.length === 0 ? (
                <div className="p-4 text-center text-xs text-commander-subtext/50">
                  {profiles.length === 0 ? "No saved profiles" : "No matches found"}
                </div>
              ) : (
                filteredProfiles.map((p) => {
                  const isSelected = selectedId === p.id;
                  const protoBadgeColor =
                    p.protocol === "SFTP"
                      ? "bg-sky-950/70 text-sky-300 border-sky-800/60"
                      : p.protocol === "SCP"
                        ? "bg-purple-950/70 text-purple-300 border-purple-800/60"
                        : "bg-amber-950/70 text-amber-300 border-amber-800/60";

                  return (
                    <div
                      key={p.id}
                      data-testid={`profile-item-${p.id}`}
                      onClick={() => handleSelectProfile(p.id)}
                      onDoubleClick={(e) => {
                        e.preventDefault();
                        handleDirectConnect(p.id);
                      }}
                      onContextMenu={(e) => handleContextMenu(e, p)}
                      className={`flex items-center justify-between p-2 rounded cursor-pointer transition-colors ${
                        isSelected
                          ? "bg-commander-selected text-white font-medium"
                          : "hover:bg-commander-dark/80 text-commander-text"
                      }`}
                    >
                      <div className="flex-1 min-w-0 pr-2">
                        <div className="flex items-center gap-1.5">
                          <span
                            className={`text-[9px] font-mono px-1 py-0.2 rounded border ${protoBadgeColor}`}
                          >
                            {p.protocol}
                          </span>
                          <span className="truncate text-xs">{p.name}</span>
                        </div>
                        <div className="text-[10px] text-commander-subtext/70 truncate mt-0.5 font-mono">
                          {p.username}@{p.host}:{p.port}
                        </div>
                      </div>

                      {p.has_saved_secret && (
                        <span title="Credentials stored in Linux Secret Service">
                          <Lock className="w-3.5 h-3.5 text-emerald-400 shrink-0" />
                        </span>
                      )}
                    </div>
                  );
                })
              )}
            </div>

            {/* Hidden file input for import */}
            <input
              ref={fileInputRef}
              type="file"
              accept=".json,application/json"
              onChange={handleImportFileSelected}
              className="hidden"
              data-testid="site-import-file-input"
            />

            {/* Sidebar Bottom Toolbar: Config (Export/Import) & Lock Site */}
            <div className="p-2 border-t border-commander-border bg-commander-surface/40 flex flex-col gap-1.5">
              <div className="flex items-center justify-between gap-1 relative">
                {/* Export / Import Menu */}
                <div className="relative" ref={configMenuRef}>
                  <button
                    type="button"
                    onClick={() => setConfigMenuOpen((prev) => !prev)}
                    className="flex items-center gap-1 px-2.5 py-1 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-xs text-commander-text hover:text-white transition-colors"
                    title="Export or Import site configurations"
                    aria-label="Configuration Menu"
                  >
                    <SlidersHorizontal className="w-3.5 h-3.5 text-commander-accent" />
                    <span>Config</span>
                    <ChevronDown className="w-3 h-3 text-commander-subtext" />
                  </button>

                  {configMenuOpen && (
                    <div
                      className="absolute left-0 bottom-full mb-1 w-44 py-1 rounded bg-commander-surface border border-commander-border shadow-xl z-50 text-xs"
                      role="menu"
                    >
                      <button
                        type="button"
                        onClick={handleOpenExport}
                        className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-commander-dark text-commander-text text-left transition-colors"
                        role="menuitem"
                      >
                        <Download className="w-3.5 h-3.5 text-sky-400" />
                        <span>Export Sites...</span>
                      </button>
                      <button
                        type="button"
                        onClick={handleTriggerImportFile}
                        className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-commander-dark text-commander-text text-left transition-colors"
                        role="menuitem"
                      >
                        <Upload className="w-3.5 h-3.5 text-emerald-400" />
                        <span>Import Sites...</span>
                      </button>
                    </div>
                  )}
                </div>

                {/* Lock Site Toggle */}
                <button
                  type="button"
                  onClick={handleToggleLock}
                  className={`flex items-center gap-1 px-2.5 py-1 rounded border text-xs transition-colors ${
                    sitesLocked
                      ? "bg-amber-500/20 text-amber-300 border-amber-500/60 hover:bg-amber-500/30 font-medium"
                      : "bg-commander-dark border-commander-border text-commander-text hover:bg-commander-selected hover:text-white"
                  }`}
                  title={
                    sitesLocked
                      ? "Site configurations are locked. Click to allow editing."
                      : "Lock site configurations to prevent edits or deletions."
                  }
                  aria-label={sitesLocked ? "Unlock Sites" : "Lock Site"}
                >
                  {sitesLocked ? (
                    <>
                      <Lock className="w-3.5 h-3.5 text-amber-400" />
                      <span>Locked</span>
                    </>
                  ) : (
                    <>
                      <Unlock className="w-3.5 h-3.5 text-commander-subtext" />
                      <span>Lock Site</span>
                    </>
                  )}
                </button>
              </div>

              {/* Keyring status footer in sidebar */}
              <div className="text-[10px] text-commander-subtext/80 flex items-center justify-between pt-0.5">
                <div className="flex items-center gap-1">
                  {keyringAvailable ? (
                    <Shield className="w-3.5 h-3.5 text-emerald-400" />
                  ) : (
                    <ShieldAlert className="w-3.5 h-3.5 text-rose-400" />
                  )}
                  <span>{keyringAvailable ? "Secret Service: Active" : "Keyring: Inaccessible"}</span>
                </div>
              </div>
            </div>
          </div>

          {/* Right Main Pane: Profile Editor */}
          <div className="flex-1 flex flex-col min-h-0 bg-commander-surface overflow-y-auto p-4 space-y-4">
            {feedbackMessage && (
              <div className="flex items-center justify-between gap-2 p-2.5 rounded bg-emerald-950/80 border border-emerald-800 text-xs text-emerald-200">
                <div className="flex items-center gap-2">
                  <Check className="w-4 h-4 shrink-0 text-emerald-400" />
                  <span>{feedbackMessage}</span>
                </div>
                <button
                  type="button"
                  onClick={() => setFeedbackMessage(null)}
                  className="p-0.5 hover:text-white text-emerald-300"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            )}

            {sitesLocked && formProfile && (
              <div className="flex items-center gap-2 p-2.5 rounded bg-amber-950/60 border border-amber-600/70 text-xs text-amber-200">
                <Lock className="w-4 h-4 shrink-0 text-amber-400" />
                <span>
                  Site configurations are locked. Click &apos;Locked&apos; at the bottom of the left column to unlock and make changes.
                </span>
              </div>
            )}

            {errorMessage && (
              <div className="flex items-center gap-2 p-2.5 rounded bg-rose-950/80 border border-rose-800 text-xs text-rose-200">
                <AlertCircle className="w-4 h-4 shrink-0 text-rose-400" />
                <span>{errorMessage}</span>
              </div>
            )}

            {!formProfile ? (
              <div className="h-full flex flex-col items-center justify-center text-center text-commander-subtext/60 p-8">
                <Server className="w-12 h-12 mb-3 opacity-30 text-commander-accent" />
                <h4 className="text-sm font-medium text-commander-text">No Profile Selected</h4>
                <p className="text-xs max-w-sm mt-1">
                  Select an existing profile on the left or create a new profile to configure host,
                  protocol, and secure credentials.
                </p>
              </div>
            ) : (
              <fieldset disabled={sitesLocked} className="space-y-4 text-xs border-0 p-0 m-0 disabled:opacity-85">
                {/* Section 1: General & Host */}
                <div className="rounded border border-commander-border p-3 space-y-3 bg-commander-darker/40">
                  <h3 className="font-semibold text-commander-accent text-[11px] uppercase tracking-wider">
                    General Connection
                  </h3>

                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="block text-commander-subtext mb-1">Profile Name</label>
                      <input
                        ref={nameInputRef}
                        type="text"
                        value={formProfile.name}
                        onChange={(e) => setFormProfile({ ...formProfile, name: e.target.value })}
                        placeholder="My Production Server"
                        className="w-full px-2.5 py-1 text-xs bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                      />
                    </div>

                    <div>
                      <label className="block text-commander-subtext mb-1">Protocol</label>
                      <select
                        value={formProfile.protocol}
                        onChange={(e) => handleProtocolChange(e.target.value as ProtocolType)}
                        className="w-full px-2.5 py-1 text-xs bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                      >
                        <option value="SFTP">SFTP (SSH File Transfer Protocol)</option>
                        <option value="SCP">SCP (Secure Copy Protocol)</option>
                        <option value="FTP">FTP (File Transfer Protocol)</option>
                        <option value="FTPS">FTPS (FTP over TLS/SSL)</option>
                      </select>
                    </div>
                  </div>

                  <div className="grid grid-cols-3 gap-3">
                    <div className="col-span-2">
                      <label className="block text-commander-subtext mb-1">Host / IP</label>
                      <input
                        type="text"
                        value={formProfile.host}
                        onChange={(e) => setFormProfile({ ...formProfile, host: e.target.value })}
                        placeholder="192.168.1.100 or ssh.example.com"
                        className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                      />
                    </div>

                    <div>
                      <label className="block text-commander-subtext mb-1">Port</label>
                      <input
                        type="number"
                        min="1"
                        max="65535"
                        value={formProfile.port}
                        onChange={(e) =>
                          setFormProfile({
                            ...formProfile,
                            port: parseInt(e.target.value, 10) || 22,
                          })
                        }
                        className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-commander-subtext mb-1">Username</label>
                    <input
                      type="text"
                      value={formProfile.username}
                      onChange={(e) => setFormProfile({ ...formProfile, username: e.target.value })}
                      placeholder="e.g. ubuntu, root, deploy"
                      className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                    />
                  </div>
                </div>

                {/* Section 2: Authentication */}
                <div className="rounded border border-commander-border p-3 space-y-3 bg-commander-darker/40">
                  <h3 className="font-semibold text-commander-accent text-[11px] uppercase tracking-wider">
                    Authentication & Credentials
                  </h3>

                  <div>
                    <label className="block text-commander-subtext mb-1">Logon Method</label>
                    <select
                      value={formProfile.auth_method.type}
                      onChange={(e) => handleAuthMethodTypeChange(e.target.value)}
                      className="w-full px-2.5 py-1 text-xs bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                    >
                      <option value="Password">Password (Keyring Protected)</option>
                      <option value="PublicKey">Public Key Authentication</option>
                      <option value="Agent">SSH Agent (SSH_AUTH_SOCK)</option>
                      <option value="Interactive">Keyboard-Interactive</option>
                    </select>
                  </div>

                  {/* Password specific field */}
                  {formProfile.auth_method.type === "Password" && (
                    <div>
                      <div className="flex items-center justify-between mb-1">
                        <label className="text-commander-subtext">Password</label>
                        <span className="text-[10px] text-emerald-400 flex items-center gap-1">
                          <Lock className="w-3 h-3" />
                          <span>Saved in Linux Secret Service</span>
                        </span>
                      </div>
                      <div className="relative">
                        <input
                          type={showPassword ? "text" : "password"}
                          value={formSecret}
                          onChange={(e) => setFormSecret(e.target.value)}
                          placeholder="Enter new password to update stored secret..."
                          className="w-full pl-2.5 pr-8 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                        />
                        <button
                          type="button"
                          onClick={() => setShowPassword((prev) => !prev)}
                          className="absolute right-2 top-1.5 text-commander-subtext hover:text-white"
                        >
                          {showPassword ? (
                            <EyeOff className="w-3.5 h-3.5" />
                          ) : (
                            <Eye className="w-3.5 h-3.5" />
                          )}
                        </button>
                      </div>
                    </div>
                  )}

                  {/* Public Key specific fields */}
                  {formProfile.auth_method.type === "PublicKey" && (
                    <div className="space-y-3">
                      <div>
                        <label className="block text-commander-subtext mb-1">
                          Private Key File Path
                        </label>
                        <input
                          type="text"
                          value={formProfile.auth_method.details.private_key_path}
                          onChange={(e) => {
                            if (formProfile.auth_method.type === "PublicKey") {
                              setFormProfile({
                                ...formProfile,
                                auth_method: {
                                  type: "PublicKey",
                                  details: {
                                    ...formProfile.auth_method.details,
                                    private_key_path: e.target.value,
                                  },
                                },
                              });
                            }
                          }}
                          placeholder="~/.ssh/id_rsa or /home/user/.ssh/id_ed25519"
                          className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                        />
                      </div>

                      <div className="flex items-center gap-2">
                        <input
                          type="checkbox"
                          id="passphrase-check"
                          checked={formProfile.auth_method.details.requires_passphrase}
                          onChange={(e) => {
                            if (formProfile.auth_method.type === "PublicKey") {
                              setFormProfile({
                                ...formProfile,
                                auth_method: {
                                  type: "PublicKey",
                                  details: {
                                    ...formProfile.auth_method.details,
                                    requires_passphrase: e.target.checked,
                                  },
                                },
                              });
                            }
                          }}
                          className="rounded border-commander-border bg-commander-dark text-commander-accent"
                        />
                        <label
                          htmlFor="passphrase-check"
                          className="text-xs text-commander-text cursor-pointer"
                        >
                          Private key is passphrase protected
                        </label>
                      </div>

                      {formProfile.auth_method.details.requires_passphrase && (
                        <div>
                          <div className="flex items-center justify-between mb-1">
                            <label className="text-commander-subtext">Passphrase</label>
                            <span className="text-[10px] text-emerald-400 flex items-center gap-1">
                              <Key className="w-3 h-3" />
                              <span>Stored in Linux Secret Service</span>
                            </span>
                          </div>
                          <input
                            type="password"
                            value={formSecret}
                            onChange={(e) => setFormSecret(e.target.value)}
                            placeholder="Enter passphrase..."
                            className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                          />
                        </div>
                      )}
                    </div>
                  )}

                  {/* Agent specific notice */}
                  {formProfile.auth_method.type === "Agent" && (
                    <div className="p-2 bg-commander-dark border border-commander-border rounded text-commander-subtext text-[11px]">
                      Authentication uses keys loaded into SSH Agent via <code>$SSH_AUTH_SOCK</code>
                      .
                    </div>
                  )}

                  {/* Interactive notice */}
                  {formProfile.auth_method.type === "Interactive" && (
                    <div className="p-2 bg-commander-dark border border-commander-border rounded text-commander-subtext text-[11px]">
                      Credentials and multi-factor challenge responses will be requested
                      interactively during connection setup.
                    </div>
                  )}
                </div>

                {/* Section 3: Initial Directories */}
                <div className="rounded border border-commander-border p-3 space-y-3 bg-commander-darker/40">
                  <h3 className="font-semibold text-commander-accent text-[11px] uppercase tracking-wider">
                    Startup Directories
                  </h3>

                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="block text-commander-subtext mb-1">
                        Initial Local Directory
                      </label>
                      <input
                        type="text"
                        value={formProfile.initial_local_directory || ""}
                        onChange={(e) =>
                          setFormProfile({
                            ...formProfile,
                            initial_local_directory: e.target.value.trim() || undefined,
                          })
                        }
                        placeholder="Default: User Home (~)"
                        className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                      />
                    </div>

                    <div>
                      <label className="block text-commander-subtext mb-1">
                        Initial Remote Directory
                      </label>
                      <input
                        type="text"
                        value={formProfile.initial_remote_directory || ""}
                        onChange={(e) =>
                          setFormProfile({
                            ...formProfile,
                            initial_remote_directory: e.target.value.trim() || undefined,
                          })
                        }
                        placeholder="e.g. /var/www or /home/ubuntu"
                        className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                      />
                    </div>
                  </div>
                </div>

                {/* Section 4: Sudo Elevation & Settings */}
                <div className="rounded border border-commander-border p-3 space-y-3 bg-commander-darker/40">
                  <h3 className="font-semibold text-commander-accent text-[11px] uppercase tracking-wider">
                    Sudo Elevation & Timeouts
                  </h3>

                  <div className="space-y-2">
                    <div className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        id="sudo-sftp-check"
                        disabled={formProfile.protocol !== "SFTP"}
                        checked={formProfile.sudo_sftp.enabled}
                        onChange={(e) =>
                          setFormProfile({
                            ...formProfile,
                            sudo_sftp: {
                              ...formProfile.sudo_sftp,
                              enabled: e.target.checked,
                            },
                          })
                        }
                        className="rounded border-commander-border bg-commander-dark text-commander-accent disabled:opacity-40"
                      />
                      <label
                        htmlFor="sudo-sftp-check"
                        className={`text-xs ${
                          formProfile.protocol === "SFTP"
                            ? "text-commander-text cursor-pointer"
                            : "text-commander-subtext/40 cursor-not-allowed"
                        }`}
                      >
                        Enable Sudo-SFTP External Server Elevation
                      </label>
                    </div>

                    {formProfile.protocol !== "SFTP" && (
                      <p className="text-[10px] text-amber-400/80">
                        Sudo elevation requires SFTP protocol (channel command execution).
                      </p>
                    )}

                    {formProfile.sudo_sftp.enabled && formProfile.protocol === "SFTP" && (
                      <div className="space-y-2.5 pt-1">
                        <div>
                          <label className="block text-commander-subtext mb-1">
                            Remote SFTP Server Binary Path
                          </label>
                          <input
                            type="text"
                            placeholder="/usr/lib/openssh/sftp-server"
                            value={
                              formProfile.sudo_sftp.sftp_server_path ??
                              (formProfile.sudo_sftp.custom_command
                                ? ""
                                : "/usr/lib/openssh/sftp-server")
                            }
                            onChange={(e) => {
                              const val = e.target.value.trim();
                              setFormProfile({
                                ...formProfile,
                                sudo_sftp: {
                                  ...formProfile.sudo_sftp,
                                  sftp_server_path: val || undefined,
                                  custom_command: val ? `sudo -n ${val}` : undefined,
                                },
                              });
                            }}
                            className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                          />
                          <p className="text-[10px] text-commander-subtext mt-0.5">
                            Common paths: Ubuntu/Debian: <code>/usr/lib/openssh/sftp-server</code>,
                            RHEL/Fedora/Arch: <code>/usr/libexec/openssh/sftp-server</code>.
                          </p>
                        </div>

                        <div>
                          <label className="block text-commander-subtext mb-1">
                            Executed Sudo Command (non-interactive)
                          </label>
                          <input
                            type="text"
                            value={
                              formProfile.sudo_sftp.custom_command ??
                              (formProfile.sudo_sftp.sftp_server_path
                                ? `sudo -n ${formProfile.sudo_sftp.sftp_server_path}`
                                : "sudo -n /usr/lib/openssh/sftp-server")
                            }
                            onChange={(e) =>
                              setFormProfile({
                                ...formProfile,
                                sudo_sftp: {
                                  ...formProfile.sudo_sftp,
                                  custom_command: e.target.value.trim() || undefined,
                                },
                              })
                            }
                            className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                          />
                        </div>

                        {/* Sudoers Requirement Guidance Box */}
                        <div className="p-2.5 rounded bg-amber-950/40 border border-amber-800/60 text-[11px] text-amber-200/95 space-y-1.5">
                          <p className="font-semibold text-amber-300">
                            Server-side sudoers configuration required:
                          </p>
                          <p>
                            LinuxSCP connects the SFTP protocol stream to a dedicated sudo process.
                            Because interactive password prompts corrupt binary SFTP protocol bytes,
                            the remote user must be permitted to run the exact binary with{" "}
                            <code>NOPASSWD</code>.
                          </p>
                          <div className="font-mono text-[10px] bg-black/50 p-1.5 rounded border border-amber-700/40 select-all">
                            {formProfile.username || "&lt;user&gt;"} ALL=(root) NOPASSWD:{" "}
                            {formProfile.sudo_sftp.sftp_server_path ||
                              (formProfile.sudo_sftp.custom_command
                                ? formProfile.sudo_sftp.custom_command.replace(
                                    /^sudo\s+(-n\s+)?/,
                                    ""
                                  )
                                : "/usr/lib/openssh/sftp-server")}
                          </div>
                          <p className="text-[10px] text-amber-300/80">
                            ⚠️ Never configure unrestricted <code>NOPASSWD: ALL</code>. LinuxSCP
                            does not modify remote sudoers automatically. OpenSSH{" "}
                            <code>internal-sftp</code> daemon mode cannot be elevated.
                          </p>
                        </div>
                      </div>
                    )}
                  </div>

                  <div className="grid grid-cols-2 gap-3 pt-2">
                    <div>
                      <label className="block text-commander-subtext mb-1">
                        Connection Timeout (s)
                      </label>
                      <input
                        type="number"
                        min="5"
                        max="300"
                        value={formProfile.connection_settings?.timeout_seconds ?? 30}
                        onChange={(e) =>
                          setFormProfile({
                            ...formProfile,
                            connection_settings: {
                              ...formProfile.connection_settings,
                              timeout_seconds: parseInt(e.target.value, 10) || 30,
                            },
                          })
                        }
                        className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                      />
                    </div>

                    <div>
                      <label className="block text-commander-subtext mb-1">
                        Keepalive Interval (s)
                      </label>
                      <input
                        type="number"
                        min="0"
                        max="300"
                        value={formProfile.connection_settings?.keepalive_interval_seconds ?? 15}
                        onChange={(e) =>
                          setFormProfile({
                            ...formProfile,
                            connection_settings: {
                              ...formProfile.connection_settings,
                              keepalive_interval_seconds: parseInt(e.target.value, 10) || undefined,
                            },
                          })
                        }
                        className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                      />
                    </div>
                  </div>
                </div>
              </fieldset>
            )}
          </div>
        </div>

        {/* Modal Footer */}
        <div className="flex items-center justify-between px-4 py-3 bg-commander-darker border-t border-commander-border">
          <div className="text-[11px] text-commander-subtext">
            <span>Passkeys & secrets strictly isolated in Linux Secret Service</span>
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onClose}
              disabled={isSaving}
              className="px-3 py-1.5 text-xs rounded border border-commander-border hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
            >
              Cancel
            </button>

            <button
              type="button"
              onClick={() => handleSave("terminal")}
              disabled={
                !formProfile ||
                isSaving ||
                (formProfile.protocol !== "SFTP" && formProfile.protocol !== "SCP")
              }
              title={
                formProfile && formProfile.protocol !== "SFTP" && formProfile.protocol !== "SCP"
                  ? "External SSH terminal is only available for SFTP and SCP connections"
                  : "Save profile and launch external interactive SSH terminal"
              }
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded font-medium border border-commander-border bg-commander-dark hover:bg-commander-selected text-commander-text hover:text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              <Terminal className="w-3.5 h-3.5 text-commander-accent" />
              <span>Open Terminal</span>
            </button>

            <button
              type="button"
              onClick={() => handleSave("save")}
              disabled={!formProfile || isSaving || sitesLocked}
              title={sitesLocked ? "Site configurations are locked. Unlock to save changes." : undefined}
              className="px-3.5 py-1.5 text-xs rounded font-medium bg-commander-selected hover:bg-commander-selected/80 text-white transition-colors disabled:opacity-50"
            >
              {isSaving ? "Saving..." : "Save Profile"}
            </button>

            <button
              type="button"
              onClick={() => handleSave("connect")}
              disabled={!formProfile || isSaving}
              className="px-4 py-1.5 text-xs rounded font-medium bg-commander-accent hover:bg-commander-accent/90 text-commander-dark transition-colors disabled:opacity-50 cursor-pointer"
            >
              {isSaving ? "Saving..." : sitesLocked ? "Connect" : "Save & Connect"}
            </button>
          </div>
        </div>
      </div>

      {/* Context Menu for Connection Items */}
      {contextMenu && (
        <div
          ref={contextMenuRef}
          style={{
            top: Math.max(8, Math.min(contextMenu.y, window.innerHeight - 120)),
            left: Math.max(8, Math.min(contextMenu.x, window.innerWidth - 170)),
          }}
          className="fixed z-[60] min-w-[150px] py-1 bg-commander-surface border border-commander-border rounded shadow-2xl text-xs text-commander-text select-none animate-in fade-in-50 duration-75"
          role="menu"
          aria-label="Connection Context Menu"
        >
          <button
            type="button"
            onClick={() => {
              const target = contextMenu.profile;
              setContextMenu(null);
              handleDirectConnect(target.id);
            }}
            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-commander-dark text-left transition-colors text-emerald-400 hover:text-emerald-300"
            role="menuitem"
          >
            <Play className="w-3.5 h-3.5 fill-current" />
            <span className="font-medium">Connect</span>
          </button>

          <button
            type="button"
            onClick={() => {
              const target = contextMenu.profile;
              setContextMenu(null);
              handleSelectProfile(target.id);
              setTimeout(() => {
                nameInputRef.current?.focus();
                nameInputRef.current?.select();
              }, 50);
            }}
            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-commander-dark text-left transition-colors text-sky-400 hover:text-sky-300"
            role="menuitem"
          >
            <Edit3 className="w-3.5 h-3.5" />
            <span>Edit</span>
          </button>

          {!sitesLocked && (
            <>
              <div className="my-1 border-t border-commander-border/60" />

              <button
                type="button"
                onClick={() => {
                  const target = contextMenu.profile;
                  setContextMenu(null);
                  handlePromptDelete(target);
                }}
                className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-rose-950/40 text-left transition-colors text-rose-400 hover:text-rose-300"
                role="menuitem"
              >
                <Trash2 className="w-3.5 h-3.5" />
                <span>Delete</span>
              </button>
            </>
          )}
        </div>
      )}

      {/* Delete Confirmation Modal */}
      <ConfirmDialog
        isOpen={deleteConfirmOpen}
        title="Delete Connection Profile"
        description={`Are you sure you want to permanently delete profile "${profileToDelete?.name || formProfile?.name || ""}" and its stored keyring secret?`}
        items={profileToDelete ? [profileToDelete.name] : formProfile ? [formProfile.name] : []}
        confirmLabel="Delete Profile"
        isDestructive={true}
        isLoading={isSaving}
        onConfirm={handleDeleteConfirm}
        onCancel={() => {
          setDeleteConfirmOpen(false);
          setProfileToDelete(null);
        }}
      />

      {/* Export Configurations Modal */}
      {exportDialogOpen && (
        <div
          className="fixed inset-0 z-[70] flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 select-none"
          role="dialog"
          aria-modal="true"
          aria-labelledby="export-dialog-title"
        >
          <div className="w-full max-w-md rounded-lg border border-commander-border bg-commander-surface shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-100">
            <div className="flex items-center justify-between px-4 py-2.5 bg-commander-darker border-b border-commander-border">
              <div className="flex items-center gap-2">
                <Download className="w-4 h-4 text-commander-accent" />
                <h3 id="export-dialog-title" className="text-sm font-semibold text-commander-text">
                  Export Site Configurations
                </h3>
              </div>
              <button
                type="button"
                onClick={() => setExportDialogOpen(false)}
                className="p-1 rounded hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="p-4 space-y-3 text-xs">
              <p className="text-commander-subtext leading-relaxed">
                Export all {profiles.length} site configuration(s) into a JSON file. Secret credentials and passwords are safe in Linux Secret Service and excluded from exports.
              </p>

              <div>
                <label className="block text-commander-subtext mb-1 font-medium">
                  Save File Destination Path:
                </label>
                <input
                  type="text"
                  value={exportPath}
                  onChange={(e) => setExportPath(e.target.value)}
                  placeholder="/home/user/linuxscp-sites.json"
                  className="w-full px-2.5 py-1.5 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 px-4 py-3 bg-commander-darker border-t border-commander-border">
              <button
                type="button"
                onClick={() => setExportDialogOpen(false)}
                disabled={isExporting}
                className="px-3 py-1.5 text-xs rounded border border-commander-border hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleExecuteExport}
                disabled={isExporting || !exportPath.trim()}
                className="flex items-center gap-1.5 px-3.5 py-1.5 text-xs rounded font-medium bg-commander-accent hover:bg-commander-accent/90 text-commander-dark transition-colors disabled:opacity-50"
              >
                <Download className="w-3.5 h-3.5" />
                <span>{isExporting ? "Exporting..." : "Export Sites"}</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Import Configurations Modal */}
      {importConfirmOpen && (
        <div
          className="fixed inset-0 z-[70] flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 select-none"
          role="dialog"
          aria-modal="true"
          aria-labelledby="import-dialog-title"
        >
          <div className="w-full max-w-lg rounded-lg border border-commander-border bg-commander-surface shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-100">
            <div className="flex items-center justify-between px-4 py-2.5 bg-commander-darker border-b border-commander-border">
              <div className="flex items-center gap-2">
                <Upload className="w-4 h-4 text-emerald-400" />
                <h3 id="import-dialog-title" className="text-sm font-semibold text-commander-text">
                  Import Site Configurations
                </h3>
              </div>
              <button
                type="button"
                onClick={() => setImportConfirmOpen(false)}
                className="p-1 rounded hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="p-4 space-y-3 text-xs">
              <p className="text-commander-text leading-relaxed">
                Found <span className="font-semibold text-commander-accent">{pendingImportCount}</span> site configuration(s) in <span className="font-mono text-commander-subtext font-semibold">{pendingImportFileName}</span>.
              </p>

              <p className="text-commander-subtext leading-relaxed">
                Choose how you would like to import these site configurations:
              </p>

              <div className="space-y-2 pt-1">
                <div className="p-3 rounded border border-commander-border bg-commander-darker/60 flex items-start gap-2">
                  <span className="font-semibold text-sky-400 shrink-0 mt-0.5">Merge:</span>
                  <span className="text-commander-subtext">
                    Add new sites and update matching sites. Any existing non-conflicting sites remain unchanged.
                  </span>
                </div>

                <div className="p-3 rounded border border-rose-900/60 bg-rose-950/20 flex items-start gap-2">
                  <span className="font-semibold text-rose-400 shrink-0 mt-0.5">Replace:</span>
                  <span className="text-rose-200/80">
                    WARNING: Removes all existing sites currently in Site Manager and replaces them completely with the imported file.
                  </span>
                </div>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 px-4 py-3 bg-commander-darker border-t border-commander-border">
              <button
                type="button"
                onClick={() => setImportConfirmOpen(false)}
                disabled={isImporting}
                className="px-3 py-1.5 text-xs rounded border border-commander-border hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
              >
                Cancel
              </button>

              <button
                type="button"
                onClick={() => handleExecuteImport("replace")}
                disabled={isImporting}
                className="px-3.5 py-1.5 text-xs rounded font-medium border border-rose-800 bg-rose-950/60 hover:bg-rose-900 text-rose-200 transition-colors disabled:opacity-50"
              >
                {isImporting ? "Importing..." : "Replace Existing"}
              </button>

              <button
                type="button"
                onClick={() => handleExecuteImport("merge")}
                disabled={isImporting}
                className="px-3.5 py-1.5 text-xs rounded font-medium bg-commander-accent hover:bg-commander-accent/90 text-commander-dark transition-colors disabled:opacity-50 font-semibold"
              >
                {isImporting ? "Importing..." : "Merge Sites"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
