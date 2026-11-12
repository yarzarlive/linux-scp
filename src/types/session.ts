import { ProtocolCapabilities } from "./capabilities";
import { ProtocolType } from "./connection";

export type SessionStatus =
  | { status: "Disconnected" }
  | { status: "Connecting" }
  | { status: "Connected" }
  | { status: "Reconnecting" }
  | { status: "Failed"; details: { error_message: string } };

export type ElevationState =
  { type: "None" } | { type: "ElevatedSudo"; effective_user: string; executed_command: string };

export interface KnownHostEntry {
  host: string;
  port: number;
  algorithm: string;
  fingerprint_sha256: string;
  public_key_openssh: string;
  trusted_at_ms: number;
}

export interface ConnectionSession {
  session_id: string;
  profile_id?: string;
  protocol: ProtocolType;
  host: string;
  port: number;
  username: string;
  status: SessionStatus;
  elevation: ElevationState;
  current_remote_path: string;
  capabilities: ProtocolCapabilities;
  last_active_ms: number;
}

export type ConnectOutcome =
  | {
      outcome: "Connected";
      session: ConnectionSession;
      trusted_host_key: KnownHostEntry;
    }
  | {
      outcome: "HostKeyUnknown";
      host: string;
      port: number;
      algorithm: string;
      fingerprint_sha256: string;
      public_key_openssh: string;
    }
  | {
      outcome: "HostKeyChanged";
      host: string;
      port: number;
      expected_algorithm: string;
      expected_fingerprint: string;
      presented_algorithm: string;
      presented_fingerprint: string;
    };

export interface SessionStateSnapshot {
  status: SessionStatus;
  session?: ConnectionSession | null;
  trusted_host_key?: KnownHostEntry | null;
}
