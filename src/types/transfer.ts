export type TransferDirection = "upload" | "download";

export type ConflictPolicy = "ask" | "overwrite" | "overwrite_if_newer" | "skip" | "rename";

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
  created_at_ms: number;
  updated_at_ms: number;
}
