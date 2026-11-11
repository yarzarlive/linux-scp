import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface LocalItemInfo {
  name: string;
  is_dir: boolean;
  size: number;
}

interface ProbeResult {
  success: boolean;
  platform: string;
  message: string;
  sample_items: LocalItemInfo[];
}

export function App() {
  const [path, setPath] = useState("/usr");
  const [result, setResult] = useState<ProbeResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleProbe() {
    setError(null);
    try {
      const data = await invoke<ProbeResult>("probe_environment", { dirPath: path });
      setResult(data);
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <main className="container">
      <h1>LinuxSCP Feasibility Spike — Tauri 2 Bridge</h1>
      <p>Testing typed React + TypeScript IPC invocation of Rust command</p>

      <div className="row">
        <input
          value={path}
          onChange={(e) => setPath(e.currentTarget.value)}
          placeholder="/usr"
        />
        <button type="button" onClick={handleProbe}>
          Probe Directory
        </button>
      </div>

      {error && <div style={{ color: "red", marginTop: "1rem" }}>Error: {error}</div>}

      {result && (
        <div style={{ marginTop: "1rem", textAlign: "left" }}>
          <h3>Platform: {result.platform}</h3>
          <p>{result.message}</p>
          <ul>
            {result.sample_items.map((item) => (
              <li key={item.name}>
                {item.is_dir ? "📁" : "📄"} {item.name} ({item.size} bytes)
              </li>
            ))}
          </ul>
        </div>
      )}
    </main>
  );
}

export default App;
