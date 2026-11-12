import { describe, expect, it } from "vitest";
import {
  createInitialRemoteNavState,
  pushRemoteHistory,
  stepRemoteHistoryBack,
  stepRemoteHistoryForward,
} from "../../src/utils/remoteNavigation";

describe("remoteNavigation history helpers", () => {
  it("pushes paths, avoids consecutive duplicates, and truncates forward stack on branch", () => {
    let state = createInitialRemoteNavState("/home/ubuntu");
    expect(state.history).toEqual(["/home/ubuntu"]);
    expect(state.historyIndex).toBe(0);

    // Consecutive duplicate is ignored
    state = pushRemoteHistory(state, "/home/ubuntu");
    expect(state.history).toEqual(["/home/ubuntu"]);
    expect(state.historyIndex).toBe(0);

    state = pushRemoteHistory(state, "/var/www");
    state = pushRemoteHistory(state, "/etc/nginx");
    expect(state.history).toEqual(["/home/ubuntu", "/var/www", "/etc/nginx"]);
    expect(state.historyIndex).toBe(2);

    // Step back once -> /var/www
    const back1 = stepRemoteHistoryBack(state);
    expect(back1.targetPath).toBe("/var/www");
    state = back1.state;
    expect(state.historyIndex).toBe(1);

    // Navigating to a new path truncates /etc/nginx
    state = pushRemoteHistory(state, "/opt/app");
    expect(state.history).toEqual(["/home/ubuntu", "/var/www", "/opt/app"]);
    expect(state.historyIndex).toBe(2);

    // Forward at end returns null
    const fwdAtEnd = stepRemoteHistoryForward(state);
    expect(fwdAtEnd.targetPath).toBeNull();
  });

  it("clamps back and forward at boundaries", () => {
    let state = createInitialRemoteNavState("/");
    const backAtStart = stepRemoteHistoryBack(state);
    expect(backAtStart.targetPath).toBeNull();
    expect(backAtStart.state.historyIndex).toBe(0);

    state = pushRemoteHistory(state, "/var");
    const back1 = stepRemoteHistoryBack(state);
    expect(back1.targetPath).toBe("/");

    const fwd1 = stepRemoteHistoryForward(back1.state);
    expect(fwd1.targetPath).toBe("/var");
    expect(fwd1.state.historyIndex).toBe(1);
  });
});
