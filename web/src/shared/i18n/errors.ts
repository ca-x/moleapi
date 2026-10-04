import { liveTranslation, message, translateCopy } from "./index";
import type { LocalizedCopy } from "./index";

export type ErrorCopy = string | LocalizedCopy;

/** Only explicitly app-created errors may carry translation metadata. */
export class AppError extends Error {
  readonly copy?: LocalizedCopy;
  constructor(copy: ErrorCopy) {
    super(translateCopy(copy));
    if (typeof copy !== "string") {
      this.copy = copy;
      Object.defineProperty(this, "message", { get: () => translateCopy(copy) });
    }
  }
}

/** App-owned failures keep their source key, including through async catches. */
export class LocalizedError extends AppError {
  constructor(key: string, values?: Record<string, unknown>) {
    super(message(key, values));
  }
}

/** Foreign errors/text remain exact, even if their message matches a catalog key. */
export function errorCopy(error: unknown): ErrorCopy {
  if (error instanceof AppError && error.copy) return error.copy;
  return error instanceof Error ? error.message : String(error);
}

export function liveError(error: unknown) {
  const copy = errorCopy(error);
  return typeof copy === "string" ? copy : liveTranslation(copy.key, copy.values);
}
