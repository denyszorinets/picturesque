import type { ApiErrorCode } from "./generated/ApiErrorCode";

/** A failed command, mirroring `picturesque_app::api::ApiError`. */
export class ApiError extends Error {
  readonly code: ApiErrorCode;

  constructor(code: ApiErrorCode, message: string) {
    super(message);
    this.name = "ApiError";
    this.code = code;
  }

  /** Converts anything a transport rejects with into an `ApiError`. */
  static from(value: unknown): ApiError {
    if (value instanceof ApiError) return value;
    if (isApiErrorShape(value)) return new ApiError(value.code, value.message);
    return new ApiError(
      "internal",
      value instanceof Error ? value.message : String(value),
    );
  }
}

const CODES: readonly string[] = [
  "unknown_command",
  "invalid_arguments",
  "not_found",
  "internal",
];

function isApiErrorShape(
  value: unknown,
): value is { code: ApiErrorCode; message: string } {
  if (typeof value !== "object" || value === null) return false;
  const { code, message } = value as Record<string, unknown>;
  return (
    typeof code === "string" &&
    CODES.includes(code) &&
    typeof message === "string"
  );
}
