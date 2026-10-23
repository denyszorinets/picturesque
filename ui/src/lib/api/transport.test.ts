import { describe, expect, it, vi } from "vitest";
import { ApiError } from "./error";
import { httpTransport, selectTransport, tauriTransport } from "./transport";

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

describe("httpTransport", () => {
  it("posts the command with JSON arguments and returns the result", async () => {
    const fetchFn = vi
      .fn<typeof fetch>()
      .mockResolvedValue(jsonResponse(200, { ok: true }));

    const result = await httpTransport(fetchFn).call("do_thing", { id: 7 });

    expect(result).toEqual({ ok: true });
    expect(fetchFn).toHaveBeenCalledWith("/api/commands/do_thing", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: '{"id":7}',
    });
  });

  it("sends an empty body when there are no arguments", async () => {
    const fetchFn = vi
      .fn<typeof fetch>()
      .mockResolvedValue(jsonResponse(200, {}));

    await httpTransport(fetchFn).call("app_info");

    expect(fetchFn.mock.calls[0]?.[1]?.body).toBe("");
  });

  it("throws the server's ApiError on failure", async () => {
    const fetchFn = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        jsonResponse(404, { code: "unknown_command", message: "nope" }),
      );

    const error = await httpTransport(fetchFn)
      .call("nope")
      .catch((e: unknown) => e);

    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ code: "unknown_command", message: "nope" });
  });

  it("reports a non-JSON failure as an internal error", async () => {
    const fetchFn = vi
      .fn<typeof fetch>()
      .mockResolvedValue(
        new Response("Bad Gateway", { status: 502, statusText: "Bad Gateway" }),
      );

    await expect(httpTransport(fetchFn).call("app_info")).rejects.toMatchObject(
      {
        code: "internal",
        message: "HTTP 502 Bad Gateway",
      },
    );
  });

  it("reports a network failure as an internal error", async () => {
    const fetchFn = vi
      .fn<typeof fetch>()
      .mockRejectedValue(new TypeError("Failed to fetch"));

    await expect(httpTransport(fetchFn).call("app_info")).rejects.toMatchObject(
      {
        code: "internal",
        message: "Failed to fetch",
      },
    );
  });
});

describe("httpTransport events", () => {
  class FakeEventSource {
    url: string;
    onmessage: ((m: MessageEvent<string>) => void) | null = null;
    closed = false;
    constructor(url: string) {
      this.url = url;
    }
    close() {
      this.closed = true;
    }
    send(data: string) {
      this.onmessage?.({ data } as MessageEvent<string>);
    }
  }

  it("delivers parsed events from /api/events and stops on unsubscribe", () => {
    let source: FakeEventSource | undefined;
    const transport = httpTransport(fetch, "", (url) => {
      source = new FakeEventSource(url);
      return source as unknown as EventSource;
    });
    const handler = vi.fn();

    const stop = transport.subscribe(handler);
    source!.send('{"type":"import_progress","run_id":3,"state":"copying"}');
    source!.send("not json");
    stop();

    expect(source!.url).toBe("/api/events");
    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith({
      type: "import_progress",
      run_id: 3,
      state: "copying",
    });
    expect(source!.closed).toBe(true);
  });
});

describe("imageUrl", () => {
  it("points at the development server's image route", () => {
    expect(httpTransport(fetch, "").imageUrl(12, "grid")).toBe(
      "/api/images/grid/12",
    );
    expect(httpTransport(fetch, "http://x").imageUrl(3, "preview")).toBe(
      "http://x/api/images/preview/3",
    );
  });

  it("adds the file of a RAW+JPEG pair when one is chosen", () => {
    expect(httpTransport(fetch, "").imageUrl(12, "full", "raw")).toBe(
      "/api/images/full/12/raw",
    );
    expect(httpTransport(fetch, "").imageUrl(12, "grid", "image")).toBe(
      "/api/images/grid/12/image",
    );
  });
});

describe("selectTransport", () => {
  it("uses Tauri IPC inside the desktop shell", () => {
    expect(selectTransport(true)).toBe(tauriTransport);
  });

  it("uses HTTP in a browser", () => {
    expect(selectTransport(false)).not.toBe(tauriTransport);
  });
});
