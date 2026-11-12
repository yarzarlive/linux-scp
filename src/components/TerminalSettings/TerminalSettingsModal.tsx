import React, { useState, useEffect, useCallback } from "react";
import {
  Terminal,
  CheckCircle2,
  AlertTriangle,
  AlertCircle,
  ShieldCheck,
  X,
  RefreshCw,
} from "lucide-react";
import { IpcClient } from "../../services/ipc";
import {
  SupportedTerminalInfo,
  TerminalEnvironmentInfo,
  TerminalPreference,
  TerminalPreferenceMode,
} from "../../types/terminal";

interface TerminalSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSaved?: (preference: TerminalPreference) => void;
}

export const TerminalSettingsModal: React.FC<TerminalSettingsModalProps> = ({
  isOpen,
  onClose,
  onSaved,
}) => {
  const [envInfo, setEnvInfo] = useState<TerminalEnvironmentInfo | null>(null);
  const [mode, setMode] = useState<TerminalPreferenceMode>("auto");
  const [presetBinary, setPresetBinary] = useState<string>("x-terminal-emulator");
  const [customExecutable, setCustomExecutable] = useState<string>("");
  const [customArgsInput, setCustomArgsInput] = useState<string>("-e");
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const loadEnvironment = useCallback(async () => {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const info = await IpcClient.getTerminalEnvironment();
      setEnvInfo(info);
      setMode(info.preference.mode);

      const firstInstalled =
        info.detected_default?.binary ||
        info.supported_terminals[0]?.binary ||
        "x-terminal-emulator";

      setPresetBinary(info.preference.preset_binary || firstInstalled);
      setCustomExecutable(info.preference.custom_executable || "");
      setCustomArgsInput(
        info.preference.custom_args_prefix.length > 0
          ? info.preference.custom_args_prefix.join(" ")
          : "-e"
      );
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String((err as { message: unknown }).message)
          : "Failed to inspect terminal environment";
      setErrorMessage(msg);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      loadEnvironment();
    }
  }, [isOpen, loadEnvironment]);

  if (!isOpen) return null;

  const handleSave = async () => {
    setErrorMessage(null);

    if (mode === "preset" && !presetBinary.trim()) {
      setErrorMessage("Please select a supported terminal emulator.");
      return;
    }

    if (mode === "custom" && !customExecutable.trim()) {
      setErrorMessage("Custom terminal executable cannot be empty.");
      return;
    }

    const parsedArgs = customArgsInput
      .split(/\s+/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);

    const nextPreference: TerminalPreference = {
      mode,
      preset_binary: mode === "preset" ? presetBinary.trim() : null,
      custom_executable: mode === "custom" ? customExecutable.trim() : null,
      custom_args_prefix:
        mode === "custom" ? (parsedArgs.length > 0 ? parsedArgs : ["-e"]) : ["-e"],
    };

    setIsSaving(true);
    try {
      const saved = await IpcClient.saveTerminalPreference(nextPreference);
      onSaved?.(saved);
      onClose();
    } catch (err: unknown) {
      const msg =
        err && typeof err === "object" && "message" in err
          ? String((err as { message: unknown }).message)
          : "Failed to save terminal preference";
      setErrorMessage(msg);
    } finally {
      setIsSaving(false);
    }
  };

  const selectedPresetInfo: SupportedTerminalInfo | undefined = envInfo?.supported_terminals.find(
    (t) => t.binary === presetBinary
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 select-none"
      role="dialog"
      aria-modal="true"
      aria-labelledby="terminal-settings-title"
    >
      <div className="w-full max-w-2xl rounded-lg border border-commander-border bg-commander-surface shadow-2xl flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-2.5 bg-commander-darker border-b border-commander-border">
          <div className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-commander-accent" />
            <h2 id="terminal-settings-title" className="text-sm font-semibold text-commander-text">
              External SSH Terminal Settings
            </h2>
          </div>
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={loadEnvironment}
              disabled={isLoading}
              className="p-1 rounded hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
              title="Refresh terminal detection"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? "animate-spin" : ""}`} />
            </button>
            <button
              type="button"
              onClick={onClose}
              className="p-1 rounded hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
              title="Close dialog"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="p-4 space-y-4 text-xs max-h-[75vh] overflow-y-auto">
          {errorMessage && (
            <div
              role="alert"
              className="flex items-center gap-2 p-2.5 rounded bg-rose-950/80 border border-rose-800 text-rose-200"
            >
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-400" />
              <span>{errorMessage}</span>
            </div>
          )}

          {/* System OpenSSH Status */}
          <div className="rounded border border-commander-border bg-commander-darker/50 p-3 flex items-center justify-between">
            <div className="space-y-0.5">
              <div className="font-semibold text-commander-text">System OpenSSH Client</div>
              <div className="text-[11px] text-commander-subtext">
                Reuses your normal <code>~/.ssh/config</code>, <code>~/.ssh/known_hosts</code>, and{" "}
                <code>SSH_AUTH_SOCK</code> agent
              </div>
            </div>
            <div className="flex items-center gap-1.5 font-mono text-[11px]">
              {envInfo?.ssh_binary_path ? (
                <span className="flex items-center gap-1 text-emerald-400 bg-emerald-950/50 border border-emerald-800/60 px-2 py-0.5 rounded">
                  <CheckCircle2 className="w-3.5 h-3.5" />
                  <span>{envInfo.ssh_binary_path}</span>
                </span>
              ) : (
                <span className="flex items-center gap-1 text-amber-300 bg-amber-950/50 border border-amber-800/60 px-2 py-0.5 rounded">
                  <AlertTriangle className="w-3.5 h-3.5" />
                  <span>ssh not found on PATH</span>
                </span>
              )}
            </div>
          </div>

          {/* Terminal Emulator Selection Mode */}
          <div className="space-y-3">
            <label className="block font-semibold text-commander-accent text-[11px] uppercase tracking-wider">
              Terminal Emulator Selection
            </label>

            {/* Option 1: Auto-detect */}
            <label
              className={`flex items-start gap-2.5 p-3 rounded border cursor-pointer transition-colors ${
                mode === "auto"
                  ? "border-commander-accent bg-commander-selected/30"
                  : "border-commander-border bg-commander-darker/40 hover:bg-commander-dark/60"
              }`}
            >
              <input
                type="radio"
                name="terminal-mode"
                value="auto"
                checked={mode === "auto"}
                onChange={() => setMode("auto")}
                className="mt-0.5"
              />
              <div className="flex-1 space-y-1">
                <div className="flex items-center justify-between">
                  <span className="font-medium text-commander-text">
                    Automatic Detection (Recommended)
                  </span>
                  {envInfo?.detected_default ? (
                    <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-emerald-950/60 border border-emerald-800/60 text-emerald-300">
                      Detected: {envInfo.detected_default.binary}
                    </span>
                  ) : (
                    <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-rose-950/60 border border-rose-800/60 text-rose-300">
                      None detected on PATH
                    </span>
                  )}
                </div>
                <p className="text-[11px] text-commander-subtext">
                  Automatically selects the first available Linux desktop terminal emulator
                  installed on your system PATH.
                </p>
              </div>
            </label>

            {/* Option 2: Supported Terminal Preset */}
            <div
              className={`p-3 rounded border transition-colors space-y-2.5 ${
                mode === "preset"
                  ? "border-commander-accent bg-commander-selected/30"
                  : "border-commander-border bg-commander-darker/40"
              }`}
            >
              <label className="flex items-start gap-2.5 cursor-pointer">
                <input
                  type="radio"
                  name="terminal-mode"
                  value="preset"
                  checked={mode === "preset"}
                  onChange={() => setMode("preset")}
                  className="mt-0.5"
                />
                <div className="flex-1">
                  <span className="font-medium text-commander-text">
                    Choose from Supported Terminal Emulators
                  </span>
                  <p className="text-[11px] text-commander-subtext mt-0.5">
                    Select a specific terminal application with pre-verified non-shell argument
                    conventions.
                  </p>
                </div>
              </label>

              {mode === "preset" && (
                <div className="pl-6 space-y-2">
                  <select
                    aria-label="Supported Terminal Emulator"
                    value={presetBinary}
                    onChange={(e) => setPresetBinary(e.target.value)}
                    className="w-full px-2.5 py-1.5 text-xs bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                  >
                    {(envInfo?.supported_terminals ?? []).map((t) => (
                      <option key={t.binary} value={t.binary}>
                        {t.display_name} {t.installed ? "— [Installed]" : "— [Not found on PATH]"}
                      </option>
                    ))}
                  </select>

                  {selectedPresetInfo && (
                    <div className="flex items-center justify-between text-[11px] px-2 py-1 rounded bg-commander-dark border border-commander-border font-mono">
                      <span>
                        Exec prefix: <code>{selectedPresetInfo.exec_prefix_args.join(" ")}</code>
                      </span>
                      {selectedPresetInfo.installed ? (
                        <span className="text-emerald-400">{selectedPresetInfo.resolved_path}</span>
                      ) : (
                        <span className="text-amber-300">Not currently installed on PATH</span>
                      )}
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Option 3: Custom Terminal Command */}
            <div
              className={`p-3 rounded border transition-colors space-y-2.5 ${
                mode === "custom"
                  ? "border-commander-accent bg-commander-selected/30"
                  : "border-commander-border bg-commander-darker/40"
              }`}
            >
              <label className="flex items-start gap-2.5 cursor-pointer">
                <input
                  type="radio"
                  name="terminal-mode"
                  value="custom"
                  checked={mode === "custom"}
                  onChange={() => setMode("custom")}
                  className="mt-0.5"
                />
                <div className="flex-1">
                  <span className="font-medium text-commander-text">
                    Custom Terminal Executable
                  </span>
                  <p className="text-[11px] text-commander-subtext mt-0.5">
                    Configure a custom terminal binary and distinct argument prefix (spawned
                    directly via OS process API without a shell).
                  </p>
                </div>
              </label>

              {mode === "custom" && (
                <div className="pl-6 grid grid-cols-3 gap-2.5">
                  <div className="col-span-2">
                    <label
                      htmlFor="custom-terminal-exec"
                      className="block text-[11px] text-commander-subtext mb-1"
                    >
                      Terminal Executable or Path
                    </label>
                    <input
                      id="custom-terminal-exec"
                      type="text"
                      value={customExecutable}
                      onChange={(e) => setCustomExecutable(e.target.value)}
                      placeholder="e.g. /usr/bin/alacritty or kitty"
                      className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                    />
                  </div>
                  <div>
                    <label
                      htmlFor="custom-terminal-args"
                      className="block text-[11px] text-commander-subtext mb-1"
                    >
                      Exec Flag Prefix
                    </label>
                    <input
                      id="custom-terminal-args"
                      type="text"
                      value={customArgsInput}
                      onChange={(e) => setCustomArgsInput(e.target.value)}
                      placeholder="-e or --"
                      className="w-full px-2.5 py-1 text-xs font-mono bg-commander-dark border border-commander-border rounded text-commander-text focus:outline-none focus:border-commander-accent"
                    />
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* Security Notice */}
          <div className="rounded border border-commander-border bg-commander-dark/80 p-2.5 flex items-start gap-2 text-[11px] text-commander-subtext">
            <ShieldCheck className="w-4 h-4 text-emerald-400 shrink-0 mt-0.5" />
            <div>
              <span className="text-commander-text font-medium">Zero-Leak Process Execution:</span>{" "}
              LinuxSCP passes <code>-p &lt;port&gt;</code>, <code>-l &lt;user&gt;</code>, optional{" "}
              <code>-i &lt;identity_file&gt;</code>, and <code>&lt;host&gt;</code> as separate OS
              process arguments. Passwords and passphrases are never exposed on the command line.
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 bg-commander-darker border-t border-commander-border">
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
            onClick={handleSave}
            disabled={isSaving || isLoading}
            className="px-4 py-1.5 text-xs rounded font-medium bg-commander-accent hover:bg-commander-accent/90 text-commander-dark transition-colors disabled:opacity-50 cursor-pointer"
          >
            {isSaving ? "Saving..." : "Save Terminal Settings"}
          </button>
        </div>
      </div>
    </div>
  );
};
