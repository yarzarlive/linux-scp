export type TerminalPreferenceMode = "auto" | "preset" | "custom";

export interface SupportedTerminalInfo {
  binary: string;
  display_name: string;
  exec_prefix_args: string[];
  installed: boolean;
  resolved_path?: string | null;
}

export interface TerminalPreference {
  mode: TerminalPreferenceMode;
  preset_binary?: string | null;
  custom_executable?: string | null;
  custom_args_prefix: string[];
}

export interface TerminalEnvironmentInfo {
  preference: TerminalPreference;
  supported_terminals: SupportedTerminalInfo[];
  detected_default?: SupportedTerminalInfo | null;
  ssh_binary_path?: string | null;
}

export type TerminalLaunchStatus = "ProcessSpawned";

export interface TerminalLaunchResult {
  profile_id: string;
  profile_name: string;
  host: string;
  port: number;
  username: string;
  terminal_binary: string;
  pid?: number | null;
  status: TerminalLaunchStatus;
  message: string;
}
