import type { AppInfo } from "./generated/AppInfo";
import { selectTransport } from "./transport";

export { ApiError } from "./error";
export type { AppInfo } from "./generated/AppInfo";

const transport = selectTransport();

/** Returns the backend application's name and version. */
export function appInfo(): Promise<AppInfo> {
  return transport.call<AppInfo>("app_info");
}
