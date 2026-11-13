// Not yet: the red commit of NOC-177's tests.
import type { Core } from "./connect.ts";

/** What a tab reaches the others with. */
export interface Platform {
  locks: {
    request(
      name: string,
      options: { ifAvailable?: boolean; signal?: AbortSignal },
      callback: (lock: unknown) => Promise<void> | void,
    ): Promise<unknown>;
  };
  channel(name: string): {
    postMessage(message: unknown): void;
    onmessage: ((event: { data: unknown }) => void) | null;
    close(): void;
  };
  startWorker(): Core;
}

export const tabCore = async (
  _name: string,
  platform: Platform,
): Promise<Core> => platform.startWorker();
