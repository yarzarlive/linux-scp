import React, { useEffect, useMemo, useState } from "react";
import { Shield, Lock, Users, AlertCircle, CheckCircle2, X, Search } from "lucide-react";
import { FileEntry, FileOwnership } from "../../types/filesystem";
import { formatFileSize, formatTimestamp, isDirectory } from "../../utils/formatters";
import {
  PermissionBits,
  bitsToOctalString,
  bitsToSymbolicString,
  parseOctalInput,
  unixPermissionsToBits,
} from "../../utils/permissions";

interface PropertiesDialogProps {
  isOpen: boolean;
  entries: FileEntry[];
  canSetPermissions: boolean;
  canSetOwnership: boolean;
  onResolveIdentity?: (uid?: number, gid?: number) => Promise<FileOwnership>;
  onApplyPermissions: (paths: string[], octalMode: string, recursive: boolean) => Promise<void>;
  onApplyOwnership: (
    path: string,
    uid?: number,
    gid?: number,
    userName?: string,
    groupName?: string
  ) => Promise<void>;
  onClose: () => void;
}

export const PropertiesDialog: React.FC<PropertiesDialogProps> = ({
  isOpen,
  entries,
  canSetPermissions,
  canSetOwnership,
  onResolveIdentity,
  onApplyPermissions,
  onApplyOwnership,
  onClose,
}) => {
  const isMulti = entries.length > 1;
  const primaryEntry = entries[0] ?? null;
  const hasDirectory = useMemo(() => entries.some((e) => isDirectory(e.file_type)), [entries]);

  const [bits, setBits] = useState<PermissionBits>(() =>
    unixPermissionsToBits(primaryEntry?.metadata.permissions)
  );
  const [octalInput, setOctalInput] = useState<string>(() =>
    bitsToOctalString(unixPermissionsToBits(primaryEntry?.metadata.permissions))
  );
  const [octalError, setOctalError] = useState<string | null>(null);
  const [recursive, setRecursive] = useState(false);
  const [confirmRecursiveOpen, setConfirmRecursiveOpen] = useState(false);

  const [ownerInput, setOwnerInput] = useState("");
  const [groupInput, setGroupInput] = useState("");
  const [resolvedOwnership, setResolvedOwnership] = useState<FileOwnership | null>(null);
  const [isResolvingIdentity, setIsResolvingIdentity] = useState(false);

  const [isApplyingPerms, setIsApplyingPerms] = useState(false);
  const [permError, setPermError] = useState<string | null>(null);
  const [permSuccess, setPermSuccess] = useState<string | null>(null);

  const [isApplyingOwner, setIsApplyingOwner] = useState(false);
  const [ownerError, setOwnerError] = useState<string | null>(null);
  const [ownerSuccess, setOwnerSuccess] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen || !primaryEntry) return;
    const initialBits = unixPermissionsToBits(primaryEntry.metadata.permissions);
    setBits(initialBits);
    setOctalInput(bitsToOctalString(initialBits));
    setOctalError(null);
    setRecursive(false);
    setConfirmRecursiveOpen(false);
    setPermError(null);
    setPermSuccess(null);
    setOwnerError(null);
    setOwnerSuccess(null);

    const ownership = primaryEntry.metadata.ownership ?? null;
    setResolvedOwnership(ownership);
    setOwnerInput(
      ownership?.user_name ?? (ownership?.uid !== undefined ? String(ownership.uid) : "")
    );
    setGroupInput(
      ownership?.group_name ?? (ownership?.gid !== undefined ? String(ownership.gid) : "")
    );
  }, [isOpen, primaryEntry]);

  if (!isOpen || entries.length === 0 || !primaryEntry) return null;

  const extractErr = (err: unknown, fallback: string) => {
    if (err && typeof err === "object" && "message" in err) {
      return String((err as { message: unknown }).message);
    }
    if (err instanceof Error) return err.message;
    return fallback;
  };

  const toggleBit = (key: keyof PermissionBits) => {
    if (!canSetPermissions || isApplyingPerms) return;
    const next = { ...bits, [key]: !bits[key] };
    setBits(next);
    setOctalInput(bitsToOctalString(next));
    setOctalError(null);
    setPermError(null);
    setPermSuccess(null);
  };

  const handleOctalChange = (val: string) => {
    setOctalInput(val);
    setPermError(null);
    setPermSuccess(null);
    const parsed = parseOctalInput(val);
    if (parsed.valid) {
      setBits(parsed.bits);
      setOctalError(null);
    } else {
      setOctalError(parsed.error);
    }
  };

  const handleApplyPermissionsClick = async () => {
    const parsed = parseOctalInput(octalInput);
    if (!parsed.valid) {
      setOctalError(parsed.error);
      return;
    }

    if (recursive && hasDirectory && !confirmRecursiveOpen) {
      setConfirmRecursiveOpen(true);
      return;
    }

    setConfirmRecursiveOpen(false);
    setIsApplyingPerms(true);
    setPermError(null);
    setPermSuccess(null);

    try {
      const normalizedOctal = bitsToOctalString(parsed.bits);
      await onApplyPermissions(
        entries.map((e) => e.path),
        normalizedOctal,
        recursive && hasDirectory
      );
      setPermSuccess(`Permissions updated to ${normalizedOctal}.`);
    } catch (err: unknown) {
      setPermError(extractErr(err, "Failed to update permissions."));
    } finally {
      setIsApplyingPerms(false);
    }
  };

  const handleResolveIdentity = async () => {
    if (!onResolveIdentity || isMulti) return;
    const ownership = primaryEntry.metadata.ownership;
    if (ownership?.uid === undefined && ownership?.gid === undefined) return;
    setIsResolvingIdentity(true);
    setOwnerError(null);
    try {
      const resolved = await onResolveIdentity(ownership?.uid, ownership?.gid);
      setResolvedOwnership(resolved);
    } catch (err: unknown) {
      setOwnerError(extractErr(err, "Remote getent lookup unavailable."));
    } finally {
      setIsResolvingIdentity(false);
    }
  };

  const handleApplyOwnership = async () => {
    if (!canSetOwnership || isMulti) return;
    const trimmedOwner = ownerInput.trim();
    const trimmedGroup = groupInput.trim();
    if (!trimmedOwner && !trimmedGroup) {
      setOwnerError("Enter a numeric UID/GID or user/group name.");
      return;
    }

    const uid = /^\d+$/.test(trimmedOwner) ? Number(trimmedOwner) : undefined;
    const gid = /^\d+$/.test(trimmedGroup) ? Number(trimmedGroup) : undefined;
    const userName = uid === undefined && trimmedOwner ? trimmedOwner : undefined;
    const groupName = gid === undefined && trimmedGroup ? trimmedGroup : undefined;

    setIsApplyingOwner(true);
    setOwnerError(null);
    setOwnerSuccess(null);

    try {
      await onApplyOwnership(primaryEntry.path, uid, gid, userName, groupName);
      setOwnerSuccess("Ownership updated on remote server.");
    } catch (err: unknown) {
      setOwnerError(extractErr(err, "Server rejected ownership change."));
    } finally {
      setIsApplyingOwner(false);
    }
  };

  const currentOwnership = resolvedOwnership ?? primaryEntry.metadata.ownership;
  const symbolicPreview = bitsToSymbolicString(bits);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 select-none"
      role="dialog"
      aria-modal="true"
      aria-labelledby="properties-dialog-title"
    >
      <div className="w-full max-w-lg rounded-lg border border-commander-border bg-commander-surface shadow-2xl overflow-hidden text-xs">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 bg-commander-darker border-b border-commander-border">
          <div className="flex items-center gap-2 min-w-0">
            <Shield className="w-4 h-4 text-commander-accent shrink-0" />
            <h3
              id="properties-dialog-title"
              className="text-sm font-semibold text-commander-text truncate"
            >
              {isMulti
                ? `Properties (${entries.length} items selected)`
                : `Properties: ${primaryEntry.name}`}
            </h3>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="p-1 rounded hover:bg-commander-dark text-commander-subtext hover:text-white transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-4 space-y-4 max-h-[75vh] overflow-y-auto">
          {/* Target Metadata */}
          <div className="p-2.5 rounded bg-commander-dark/60 border border-commander-border/60 space-y-1">
            {isMulti ? (
              <>
                <div className="text-commander-text font-medium">
                  Applying permissions to {entries.length} selected items
                </div>
                <div className="text-[11px] text-commander-subtext max-h-16 overflow-y-auto font-mono">
                  {entries.map((e) => (
                    <div key={e.path} className="truncate">
                      {e.path}
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <>
                <div className="flex justify-between gap-2">
                  <span className="text-commander-subtext">Path:</span>
                  <span
                    className="font-mono text-commander-text truncate"
                    title={primaryEntry.path}
                  >
                    {primaryEntry.path}
                  </span>
                </div>
                <div className="flex justify-between gap-2">
                  <span className="text-commander-subtext">Type / Size:</span>
                  <span className="text-commander-text">
                    {primaryEntry.file_type.type} •{" "}
                    {formatFileSize(
                      primaryEntry.metadata.size,
                      isDirectory(primaryEntry.file_type)
                    )}
                  </span>
                </div>
                <div className="flex justify-between gap-2">
                  <span className="text-commander-subtext">Modified:</span>
                  <span className="font-mono text-commander-text">
                    {formatTimestamp(primaryEntry.metadata.modified_at_secs)}
                  </span>
                </div>
              </>
            )}
          </div>

          {/* Unix Permissions Section */}
          <div className="space-y-2.5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-1.5 font-semibold text-commander-text">
                <Lock className="w-3.5 h-3.5 text-amber-400" />
                <span>Unix Permissions</span>
              </div>
              <span
                className="font-mono text-[11px] text-commander-accent"
                data-testid="symbolic-preview"
              >
                {symbolicPreview}
              </span>
            </div>

            {!canSetPermissions && (
              <div className="flex items-center gap-2 p-2 rounded bg-amber-950/40 border border-amber-800/50 text-amber-200 text-[11px]">
                <AlertCircle className="w-3.5 h-3.5 shrink-0 text-amber-400" />
                <span>Permission changes are not supported by this session protocol.</span>
              </div>
            )}

            <div className="rounded border border-commander-border overflow-hidden">
              <table className="w-full text-center border-collapse">
                <thead className="bg-commander-darker text-[11px] text-commander-subtext">
                  <tr>
                    <th className="py-1.5 px-2 text-left">Scope</th>
                    <th className="py-1.5 px-2">Read (4)</th>
                    <th className="py-1.5 px-2">Write (2)</th>
                    <th className="py-1.5 px-2">Execute (1)</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-commander-border/40 bg-commander-dark/30">
                  <tr>
                    <td className="py-1.5 px-2 text-left font-medium text-commander-text">Owner</td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label="Owner Read"
                        checked={bits.userRead}
                        disabled={!canSetPermissions || isApplyingPerms}
                        onChange={() => toggleBit("userRead")}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label="Owner Write"
                        checked={bits.userWrite}
                        disabled={!canSetPermissions || isApplyingPerms}
                        onChange={() => toggleBit("userWrite")}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label="Owner Execute"
                        checked={bits.userExecute}
                        disabled={!canSetPermissions || isApplyingPerms}
                        onChange={() => toggleBit("userExecute")}
                      />
                    </td>
                  </tr>
                  <tr>
                    <td className="py-1.5 px-2 text-left font-medium text-commander-text">Group</td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label="Group Read"
                        checked={bits.groupRead}
                        disabled={!canSetPermissions || isApplyingPerms}
                        onChange={() => toggleBit("groupRead")}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label="Group Write"
                        checked={bits.groupWrite}
                        disabled={!canSetPermissions || isApplyingPerms}
                        onChange={() => toggleBit("groupWrite")}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label="Group Execute"
                        checked={bits.groupExecute}
                        disabled={!canSetPermissions || isApplyingPerms}
                        onChange={() => toggleBit("groupExecute")}
                      />
                    </td>
                  </tr>
                  <tr>
                    <td className="py-1.5 px-2 text-left font-medium text-commander-text">
                      Others
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label="Others Read"
                        checked={bits.otherRead}
                        disabled={!canSetPermissions || isApplyingPerms}
                        onChange={() => toggleBit("otherRead")}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label="Others Write"
                        checked={bits.otherWrite}
                        disabled={!canSetPermissions || isApplyingPerms}
                        onChange={() => toggleBit("otherWrite")}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label="Others Execute"
                        checked={bits.otherExecute}
                        disabled={!canSetPermissions || isApplyingPerms}
                        onChange={() => toggleBit("otherExecute")}
                      />
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>

            {/* Special Bits & Octal Input */}
            <div className="flex flex-wrap items-center justify-between gap-2 pt-1">
              <div className="flex items-center gap-3 text-[11px]">
                <label className="flex items-center gap-1 cursor-pointer">
                  <input
                    type="checkbox"
                    aria-label="SUID"
                    checked={bits.setUid}
                    disabled={!canSetPermissions || isApplyingPerms}
                    onChange={() => toggleBit("setUid")}
                  />
                  <span>SUID (4)</span>
                </label>
                <label className="flex items-center gap-1 cursor-pointer">
                  <input
                    type="checkbox"
                    aria-label="SGID"
                    checked={bits.setGid}
                    disabled={!canSetPermissions || isApplyingPerms}
                    onChange={() => toggleBit("setGid")}
                  />
                  <span>SGID (2)</span>
                </label>
                <label className="flex items-center gap-1 cursor-pointer">
                  <input
                    type="checkbox"
                    aria-label="Sticky Bit"
                    checked={bits.sticky}
                    disabled={!canSetPermissions || isApplyingPerms}
                    onChange={() => toggleBit("sticky")}
                  />
                  <span>Sticky (1)</span>
                </label>
              </div>

              <div className="flex items-center gap-1.5">
                <label htmlFor="octal-mode-input" className="text-[11px] text-commander-subtext">
                  Octal:
                </label>
                <input
                  id="octal-mode-input"
                  type="text"
                  value={octalInput}
                  disabled={!canSetPermissions || isApplyingPerms}
                  onChange={(e) => handleOctalChange(e.target.value)}
                  maxLength={4}
                  className="w-16 px-2 py-1 rounded bg-commander-dark border border-commander-border font-mono text-center text-commander-text focus:outline-none focus:border-commander-accent"
                />
              </div>
            </div>

            {octalError && (
              <div className="text-[11px] text-rose-400" role="alert">
                {octalError}
              </div>
            )}

            {hasDirectory && canSetPermissions && (
              <label className="flex items-center gap-2 text-[11px] text-commander-text pt-1 cursor-pointer">
                <input
                  type="checkbox"
                  checked={recursive}
                  disabled={isApplyingPerms}
                  onChange={(e) => {
                    setRecursive(e.target.checked);
                    setConfirmRecursiveOpen(false);
                  }}
                />
                <span>Apply permissions recursively to subdirectories and files</span>
              </label>
            )}

            {confirmRecursiveOpen && (
              <div className="p-2.5 rounded bg-amber-950/60 border border-amber-700/70 text-amber-100 text-[11px] space-y-2">
                <div className="font-semibold">
                  Confirm Recursive Permission Change ({bitsToOctalString(bits)})
                </div>
                <p>
                  This will recursively apply mode{" "}
                  <span className="font-mono font-semibold">{bitsToOctalString(bits)}</span> to all
                  nested files and subdirectories inside the selected directory target(s).
                </p>
                <div className="flex justify-end gap-2">
                  <button
                    type="button"
                    onClick={() => setConfirmRecursiveOpen(false)}
                    className="px-2 py-1 rounded border border-commander-border bg-commander-dark text-commander-subtext hover:text-white"
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    onClick={handleApplyPermissionsClick}
                    className="px-2.5 py-1 rounded bg-amber-600 hover:bg-amber-700 text-white font-medium"
                  >
                    Confirm Recursive Apply
                  </button>
                </div>
              </div>
            )}

            {permError && (
              <div
                className="flex items-center gap-1.5 p-2 rounded bg-rose-950/50 border border-rose-800 text-rose-300 text-[11px]"
                role="alert"
              >
                <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                <span>{permError}</span>
              </div>
            )}

            {permSuccess && (
              <div className="flex items-center gap-1.5 p-2 rounded bg-emerald-950/50 border border-emerald-800 text-emerald-300 text-[11px]">
                <CheckCircle2 className="w-3.5 h-3.5 shrink-0" />
                <span>{permSuccess}</span>
              </div>
            )}

            <div className="flex justify-end">
              <button
                type="button"
                disabled={!canSetPermissions || Boolean(octalError) || isApplyingPerms}
                onClick={handleApplyPermissionsClick}
                className="px-3 py-1 rounded bg-commander-accent hover:bg-commander-accent/90 text-commander-dark font-medium disabled:opacity-40 disabled:cursor-not-allowed"
              >
                {isApplyingPerms ? "Applying..." : "Apply Permissions"}
              </button>
            </div>
          </div>

          {/* Separate Ownership Section */}
          <div className="pt-3 border-t border-commander-border space-y-2.5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-1.5 font-semibold text-commander-text">
                <Users className="w-3.5 h-3.5 text-sky-400" />
                <span>Ownership (Owner / Group)</span>
              </div>
              {!isMulti &&
                onResolveIdentity &&
                (currentOwnership?.uid !== undefined || currentOwnership?.gid !== undefined) && (
                  <button
                    type="button"
                    onClick={handleResolveIdentity}
                    disabled={isResolvingIdentity}
                    className="flex items-center gap-1 px-2 py-0.5 rounded bg-commander-dark hover:bg-commander-selected border border-commander-border text-[11px] text-commander-subtext hover:text-white"
                  >
                    <Search className="w-3 h-3" />
                    <span>{isResolvingIdentity ? "Resolving..." : "Resolve Names (getent)"}</span>
                  </button>
                )}
            </div>

            {isMulti ? (
              <p className="text-[11px] text-commander-subtext">
                Ownership editing is unavailable for multi-selection. Select a single item to view
                or edit owner/group.
              </p>
            ) : (
              <>
                <div className="text-[11px] text-commander-subtext flex flex-wrap gap-x-4 gap-y-1">
                  <span>
                    Current Owner:{" "}
                    <span className="font-mono text-commander-text">
                      {currentOwnership?.user_name
                        ? `${currentOwnership.user_name} (${currentOwnership.uid ?? "?"})`
                        : currentOwnership?.uid !== undefined
                          ? `UID ${currentOwnership.uid}`
                          : "Unavailable"}
                    </span>
                  </span>
                  <span>
                    Current Group:{" "}
                    <span className="font-mono text-commander-text">
                      {currentOwnership?.group_name
                        ? `${currentOwnership.group_name} (${currentOwnership.gid ?? "?"})`
                        : currentOwnership?.gid !== undefined
                          ? `GID ${currentOwnership.gid}`
                          : "Unavailable"}
                    </span>
                  </span>
                </div>

                {!canSetOwnership ? (
                  <div className="flex items-center gap-2 p-2 rounded bg-amber-950/40 border border-amber-800/50 text-amber-200 text-[11px]">
                    <AlertCircle className="w-3.5 h-3.5 shrink-0 text-amber-400" />
                    <span>Ownership changes are not supported by this session protocol.</span>
                  </div>
                ) : (
                  <p className="text-[10px] text-commander-subtext/80">
                    Enter numeric UID/GID (or user/group name if remote{" "}
                    <code className="font-mono">getent</code> is available). Non-root users are
                    typically restricted by the server from changing file owner.
                  </p>
                )}

                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <label
                      htmlFor="owner-input"
                      className="block text-[11px] text-commander-subtext mb-1"
                    >
                      Owner (UID or name)
                    </label>
                    <input
                      id="owner-input"
                      type="text"
                      value={ownerInput}
                      disabled={!canSetOwnership || isApplyingOwner}
                      onChange={(e) => {
                        setOwnerInput(e.target.value);
                        setOwnerError(null);
                        setOwnerSuccess(null);
                      }}
                      placeholder="e.g. 1000 or ubuntu"
                      className="w-full px-2 py-1 rounded bg-commander-dark border border-commander-border font-mono text-commander-text focus:outline-none focus:border-commander-accent disabled:opacity-50"
                    />
                  </div>
                  <div>
                    <label
                      htmlFor="group-input"
                      className="block text-[11px] text-commander-subtext mb-1"
                    >
                      Group (GID or name)
                    </label>
                    <input
                      id="group-input"
                      type="text"
                      value={groupInput}
                      disabled={!canSetOwnership || isApplyingOwner}
                      onChange={(e) => {
                        setGroupInput(e.target.value);
                        setOwnerError(null);
                        setOwnerSuccess(null);
                      }}
                      placeholder="e.g. 1000 or www-data"
                      className="w-full px-2 py-1 rounded bg-commander-dark border border-commander-border font-mono text-commander-text focus:outline-none focus:border-commander-accent disabled:opacity-50"
                    />
                  </div>
                </div>

                {ownerError && (
                  <div
                    className="flex items-center gap-1.5 p-2 rounded bg-rose-950/50 border border-rose-800 text-rose-300 text-[11px]"
                    role="alert"
                  >
                    <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                    <span>{ownerError}</span>
                  </div>
                )}

                {ownerSuccess && (
                  <div className="flex items-center gap-1.5 p-2 rounded bg-emerald-950/50 border border-emerald-800 text-emerald-300 text-[11px]">
                    <CheckCircle2 className="w-3.5 h-3.5 shrink-0" />
                    <span>{ownerSuccess}</span>
                  </div>
                )}

                <div className="flex justify-end">
                  <button
                    type="button"
                    disabled={!canSetOwnership || isApplyingOwner}
                    onClick={handleApplyOwnership}
                    className="px-3 py-1 rounded border border-commander-border bg-commander-dark hover:bg-commander-selected text-commander-text hover:text-white font-medium disabled:opacity-40 disabled:cursor-not-allowed"
                  >
                    {isApplyingOwner ? "Applying..." : "Apply Ownership"}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex justify-end px-4 py-2.5 bg-commander-darker border-t border-commander-border">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1 rounded border border-commander-border hover:bg-commander-dark text-commander-subtext hover:text-white"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
