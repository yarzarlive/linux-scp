import React from "react";
import { ShieldAlert, ShieldQuestion, Lock, XCircle } from "lucide-react";
import { ConnectOutcome } from "../../types/session";

type HostKeyChallenge = Extract<ConnectOutcome, { outcome: "HostKeyUnknown" | "HostKeyChanged" }>;

interface HostKeyPromptModalProps {
  challenge: HostKeyChallenge | null;
  isProcessing?: boolean;
  error?: string | null;
  onApproveUnknownKey: (challenge: Extract<ConnectOutcome, { outcome: "HostKeyUnknown" }>) => void;
  onCancel: () => void;
}

export const HostKeyPromptModal: React.FC<HostKeyPromptModalProps> = ({
  challenge,
  isProcessing = false,
  error = null,
  onApproveUnknownKey,
  onCancel,
}) => {
  if (!challenge) return null;

  const isChanged = challenge.outcome === "HostKeyChanged";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4 select-none"
      role="dialog"
      aria-modal="true"
      aria-labelledby="host-key-modal-title"
    >
      <div
        className={`w-full max-w-lg rounded-lg border shadow-2xl overflow-hidden bg-commander-surface animate-in fade-in zoom-in-95 duration-100 ${
          isChanged ? "border-rose-700" : "border-amber-700/80"
        }`}
      >
        {/* Header */}
        <div
          className={`flex items-center gap-3 px-4 py-3 border-b ${
            isChanged
              ? "bg-rose-950/80 border-rose-800 text-rose-200"
              : "bg-amber-950/60 border-amber-800/60 text-amber-200"
          }`}
        >
          {isChanged ? (
            <ShieldAlert className="w-6 h-6 text-rose-400 shrink-0" />
          ) : (
            <ShieldQuestion className="w-6 h-6 text-amber-400 shrink-0" />
          )}
          <div>
            <h3 id="host-key-modal-title" className="text-sm font-semibold">
              {isChanged
                ? "SECURITY WARNING: Remote Host Identification Has Changed!"
                : "Verify Unknown SSH Host Key"}
            </h3>
            <p className="text-[11px] opacity-85 mt-0.5">
              Target host:{" "}
              <span className="font-mono font-semibold">
                {challenge.host}:{challenge.port}
              </span>
            </p>
          </div>
        </div>

        {/* Body */}
        <div className="p-4 space-y-3 text-xs text-commander-text">
          {isChanged ? (
            <>
              <div className="p-3 rounded bg-rose-950/40 border border-rose-900/70 text-rose-200 space-y-1.5">
                <p className="font-semibold">
                  Connection to {challenge.host}:{challenge.port} has been strictly blocked!
                </p>
                <p className="text-[11px] text-rose-200/85 leading-relaxed">
                  The host key presented by the server does not match the fingerprint stored in the
                  trusted known-hosts database. This could indicate a Man-in-the-Middle (MITM)
                  attack or an administrator key change. LinuxSCP never automatically replaces
                  stored host keys.
                </p>
              </div>

              <div className="space-y-2 font-mono text-[11px]">
                <div className="p-2.5 rounded bg-commander-dark border border-commander-border">
                  <div className="text-commander-subtext text-[10px] uppercase mb-1">
                    Expected Stored Key ({challenge.expected_algorithm})
                  </div>
                  <div className="text-emerald-400 break-all">{challenge.expected_fingerprint}</div>
                </div>

                <div className="p-2.5 rounded bg-commander-dark border border-rose-900/60">
                  <div className="text-rose-400 text-[10px] uppercase mb-1">
                    Presented Server Key ({challenge.presented_algorithm})
                  </div>
                  <div className="text-rose-300 break-all">{challenge.presented_fingerprint}</div>
                </div>
              </div>
            </>
          ) : (
            <>
              <p className="text-commander-subtext leading-relaxed">
                The authenticity of host{" "}
                <span className="font-mono text-commander-text font-medium">
                  {challenge.host}:{challenge.port}
                </span>{" "}
                cannot be established because it is not present in your trusted known-hosts store.
                Please verify the server fingerprint below before trusting it.
              </p>

              <div className="p-3 rounded bg-commander-dark border border-commander-border space-y-2 font-mono text-[11px]">
                <div className="flex items-center justify-between">
                  <span className="text-commander-subtext">Algorithm:</span>
                  <span className="text-commander-accent font-semibold">{challenge.algorithm}</span>
                </div>
                <div>
                  <div className="text-commander-subtext mb-1">SHA-256 Fingerprint:</div>
                  <div className="p-2 rounded bg-commander-darker border border-commander-border/70 text-emerald-400 break-all select-text">
                    {challenge.fingerprint_sha256}
                  </div>
                </div>
              </div>
            </>
          )}

          {error && (
            <div className="p-2.5 rounded bg-rose-950/60 border border-rose-800 text-rose-200 text-xs">
              {error}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 bg-commander-darker border-t border-commander-border">
          <button
            type="button"
            onClick={onCancel}
            disabled={isProcessing}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded border border-commander-border hover:bg-commander-dark text-xs text-commander-text transition-colors"
          >
            <XCircle className="w-3.5 h-3.5 text-rose-400" />
            <span>{isChanged ? "Abort & Close" : "Reject & Disconnect"}</span>
          </button>

          {!isChanged && (
            <button
              type="button"
              disabled={isProcessing}
              onClick={() => onApproveUnknownKey(challenge)}
              className="flex items-center gap-1.5 px-3.5 py-1.5 rounded bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-medium transition-colors cursor-pointer disabled:opacity-50"
            >
              <Lock className="w-3.5 h-3.5" />
              <span>{isProcessing ? "Trusting & Connecting..." : "Trust Host Key & Connect"}</span>
            </button>
          )}
        </div>
      </div>
    </div>
  );
};
