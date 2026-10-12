import { invoke, isTauri } from "@tauri-apps/api/core";
import { ApiError } from "./error";

/** Delivers a named command with JSON arguments to the backend (ADR-0010). */
export interface Transport {
  call<T>(command: string, args?: unknown): Promise<T>;
}

/** Inside the desktop app: the single `api` Tauri command. */
export const tauriTransport: Transport = {
  async call<T>(command: string, args?: unknown): Promise<T> {
    try {
      return await invoke<T>("api", { command, args: args ?? null });
    } catch (e) {
      throw ApiError.from(e);
    }
  },
};

/** In a browser: `POST /api/commands/{command}` on the development server. */
export function httpTransport(
  fetchFn: typeof fetch = fetch,
  baseUrl = "",
): Transport {
  return {
    async call<T>(command: string, args?: unknown): Promise<T> {
      let response: Response;
      try {
        response = await fetchFn(
          `${baseUrl}/api/commands/${encodeURIComponent(command)}`,
          {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: args === undefined ? "" : JSON.stringify(args),
          },
        );
      } catch (e) {
        throw ApiError.from(e);
      }

      const body: unknown = await response.json().catch(() => undefined);
      if (!response.ok) {
        throw body === undefined
          ? new ApiError(
              "internal",
              `HTTP ${response.status} ${response.statusText}`,
            )
          : ApiError.from(body);
      }
      return body as T;
    },
  };
}

/** Chooses Tauri IPC inside the desktop shell and HTTP everywhere else. */
export function selectTransport(inTauri: boolean = isTauri()): Transport {
  return inTauri ? tauriTransport : httpTransport();
}
