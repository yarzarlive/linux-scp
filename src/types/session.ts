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
