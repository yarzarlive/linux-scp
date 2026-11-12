import { SortColumn, SortDirection } from "../components/FilePanel/ColumnHeader";

export interface RemoteNavHistoryState {
  history: string[];
  historyIndex: number;
  showHidden: boolean;
  sortBy: SortColumn;
  sortDirection: SortDirection;
}

export function createInitialRemoteNavState(initialPath?: string): RemoteNavHistoryState {
  return {
    history: initialPath ? [initialPath] : [],
    historyIndex: initialPath ? 0 : -1,
    showHidden: false,
    sortBy: "name",
    sortDirection: "asc",
  };
}

export function pushRemoteHistory(
  state: RemoteNavHistoryState,
  newPath: string
): RemoteNavHistoryState {
  if (state.historyIndex >= 0 && state.history[state.historyIndex] === newPath) {
    return state;
  }
  const truncated = state.history.slice(0, state.historyIndex + 1);
  const nextHistory = [...truncated, newPath];
  return {
    ...state,
    history: nextHistory,
    historyIndex: nextHistory.length - 1,
  };
}

export function stepRemoteHistoryBack(state: RemoteNavHistoryState): {
  state: RemoteNavHistoryState;
  targetPath: string | null;
} {
  if (state.historyIndex <= 0) {
    return { state, targetPath: null };
  }
  const nextIndex = state.historyIndex - 1;
  return {
    state: { ...state, historyIndex: nextIndex },
    targetPath: state.history[nextIndex] ?? null,
  };
}

export function stepRemoteHistoryForward(state: RemoteNavHistoryState): {
  state: RemoteNavHistoryState;
  targetPath: string | null;
} {
  if (state.historyIndex >= state.history.length - 1) {
    return { state, targetPath: null };
  }
  const nextIndex = state.historyIndex + 1;
  return {
    state: { ...state, historyIndex: nextIndex },
    targetPath: state.history[nextIndex] ?? null,
  };
}
