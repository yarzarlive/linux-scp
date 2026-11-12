export type TransferDirection = "upload" | "download";

export type ConflictPolicy = "ask" | "overwrite" | "overwrite_if_newer" | "skip" | "rename";

export type ConflictDecision = "overwrite" | "overwrite_if_newer" | "skip" | "rename" | "cancel";

export type TransferState =
  | { status: "Queued" }
  | { status: "Connecting" }
  | { status: "Transferring" }
  | { status: "Verifying" }
  | { status: "Completed" }
  | { status: "Cancelled" }
  | { status: "Failed"; details: { error_message: string } };

export interface TransferJob {
  id: string;
  session_id: string;
  source_path: string;
  destination_path: string;
  direction: TransferDirection;
  state: TransferState;
  total_bytes?: number;
  transferred_bytes: number;
  speed_bps: number;
  eta_seconds?: number;
  conflict_policy: ConflictPolicy;
  is_directory?: boolean;
  total_files?: number;
  completed_files?: number;
  current_file?: string | null;
  created_at_ms: number;
  updated_at_ms: number;
}

export interface TransferProgressEvent {
  job_id: string;
  transferred_bytes: number;
  total_bytes?: number;
  speed_bps: number;
  eta_seconds?: number;
  completed_files: number;
  total_files: number;
  current_file?: string | null;
}

export interface TransferConflictPrompt {
  conflict_id: string;
  job_id: string;
  direction: TransferDirection;
  source_path: string;
  destination_path: string;
  source_size?: number;
  source_modified_at_secs?: number;
  target_size?: number;
  target_modified_at_secs?: number;
}
