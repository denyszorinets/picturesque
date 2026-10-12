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

describe("selectTransport", () => {
  it("uses Tauri IPC inside the desktop shell", () => {
    expect(selectTransport(true)).toBe(tauriTransport);
  });

  it("uses HTTP in a browser", () => {
    expect(selectTransport(false)).not.toBe(tauriTransport);
  });
});
