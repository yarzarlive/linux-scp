import { FileEntry } from "./filesystem";
import { SessionStateSnapshot } from "./session";
import { RemoteNavHistoryState, createInitialRemoteNavState } from "../utils/remoteNavigation";

/**
 * Encapsulates the independent state owned by a single remote connection tab.
 * No mutable session, navigation, or selection state is shared between tabs.
 */
export interface RemoteTabState {
  id: string;
  title: string;
  sessionState: SessionStateSnapshot;
  connectionError: string | null;
  localPath: string;
  currentPath: string;
  parentPath: string | null;
  homePath: string | null;
  entries: FileEntry[];
  selectedPaths: Set<string>;
  focusedPath?: string;
  navHistory: RemoteNavHistoryState;
  savedPaths: string[];
  lastConnectedProfileId?: string;
  canReconnect: boolean;
}

export function createDefaultTabState(
  id: string,
  title = "Remote 1",
  localPath = ""
): RemoteTabState {
  return {
    id,
    title,
    localPath,
    sessionState: {
      status: { status: "Disconnected" },
      session: null,
      trusted_host_key: null,
    },
    connectionError: null,
    currentPath: "",
    parentPath: null,
    homePath: null,
    entries: [],
    selectedPaths: new Set(),
    focusedPath: undefined,
    navHistory: createInitialRemoteNavState(),
    savedPaths: [],
    canReconnect: false,
  };
}
