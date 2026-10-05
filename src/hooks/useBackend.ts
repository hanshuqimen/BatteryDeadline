import { useCallback, useEffect, useState } from "react";
import { callBackend } from "../backend";
import type { ActionResult, AppSnapshot, CommandName } from "../types";

export function useBackend() {
  const [data, setData] = useState<AppSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [disconnected, setDisconnected] = useState(false);
  const refresh = useCallback(async () => {
    const snapshot = await callBackend<AppSnapshot>("get_snapshot");
    setData(snapshot);
    setDisconnected(false);
  }, []);
  useEffect(() => {
    let disposed = false;
    let inFlight = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (disposed || inFlight) return;
      inFlight = true;
      if (!document.hidden) {
        try {
          const snapshot = await callBackend<AppSnapshot>("get_snapshot");
          if (!disposed) {
            setData(snapshot);
            setDisconnected(false);
          }
        } catch (error) {
          if (!disposed) {
            setDisconnected(true);
            setError(String(error instanceof Error ? error.message : error));
          }
        }
      }
      inFlight = false;
      if (!disposed) timer = setTimeout(() => void poll(), document.hidden ? 15_000 : 2_000);
    };
    void poll();
    const wake = () => {
      if (!document.hidden) {
        clearTimeout(timer);
        void poll();
      }
    };
    document.addEventListener("visibilitychange", wake);
    return () => {
      disposed = true;
      clearTimeout(timer);
      document.removeEventListener("visibilitychange", wake);
    };
  }, []);
  const perform = useCallback(
    async <T = null>(
      command: CommandName,
      args?: Record<string, unknown>,
    ): Promise<ActionResult<T>> => {
      setBusy(true);
      setError(null);
      try {
        const value = await callBackend<T>(command, args);
        await refresh();
        return { ok: true, value };
      } catch (error) {
        setError(String(error instanceof Error ? error.message : error));
        return { ok: false };
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );
  return { data, error, busy, disconnected, perform, dismissError: () => setError(null) };
}
