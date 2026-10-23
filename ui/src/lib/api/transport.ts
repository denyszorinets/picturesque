import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ApiError } from "./error";
import type { AppEvent } from "./generated/AppEvent";

/** Delivers named commands to the backend and backend events to the UI (ADR-0010). */
export interface Transport {
  call<T>(command: string, args?: unknown): Promise<T>;
  /** Calls `handler` for every backend event; returns a function that stops listening. */
  subscribe(handler: (event: AppEvent) => void): () => void;
  /**
   * URL of a photo's cached image, usable as an `<img src>`. `view` picks
   * the file of a RAW+JPEG pair; without it, the `pair_view` setting does.
   */
  imageUrl(photoId: number, size: ImageSize, view?: PairView): string;
}

/** Cached image sizes (see `picturesque_app::thumbnails::ImageSize`). */
export type ImageSize = "grid" | "preview" | "full";

/** The file of a RAW+JPEG pair to show (see `picturesque_core::photo::PairView`). */
export type PairView = "image" | "raw";

/** The image request path `{size}/{photo_id}[/{view}]`. */
function imagePath(photoId: number, size: ImageSize, view?: PairView): string {
  return view ? `${size}/${photoId}/${view}` : `${size}/${photoId}`;
}

/** Name of the Tauri event carrying backend events (see src-tauri/src/lib.rs). */
export const TAURI_EVENT = "app-event";

/** Inside the desktop app: the single `api` Tauri command and the `app-event` event. */
export const tauriTransport: Transport = {
  async call<T>(command: string, args?: unknown): Promise<T> {
    try {
      return await invoke<T>("api", { command, args: args ?? null });
    } catch (e) {
      throw ApiError.from(e);
    }
  },
  subscribe(handler) {
    let stop: (() => void) | undefined;
    let stopped = false;
    void listen<AppEvent>(TAURI_EVENT, (e) => handler(e.payload)).then(
      (unlisten) => {
        if (stopped) unlisten();
        else stop = unlisten;
      },
    );
    return () => {
      stopped = true;
      stop?.();
    };
  },
  imageUrl(photoId, size, view) {
    // The `picturesque://` protocol (src-tauri/src/images.rs).
    return convertFileSrc(imagePath(photoId, size, view), "picturesque");
  },
};

/**
 * In a browser: `POST /api/commands/{command}` and server-sent events from
 * `/api/events` on the development server.
 */
export function httpTransport(
  fetchFn: typeof fetch = fetch,
  baseUrl = "",
  eventSource: (url: string) => EventSource = (url) => new EventSource(url),
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
    subscribe(handler) {
      const source = eventSource(`${baseUrl}/api/events`);
      source.onmessage = (message: MessageEvent<string>) => {
        try {
          handler(JSON.parse(message.data) as AppEvent);
        } catch {
          // Ignore malformed events rather than breaking the stream.
        }
      };
      return () => source.close();
    },
    imageUrl(photoId, size, view) {
      return `${baseUrl}/api/images/${imagePath(photoId, size, view)}`;
    },
  };
}

/** Chooses Tauri IPC inside the desktop shell and HTTP everywhere else. */
export function selectTransport(inTauri: boolean = isTauri()): Transport {
  return inTauri ? tauriTransport : httpTransport();
}
