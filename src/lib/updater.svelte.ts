import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "uptodate"
  | "error";

interface UpdateState {
  status: UpdateStatus;
  /** Version offered by the endpoint, once known. */
  version: string | null;
  currentVersion: string | null;
  notes: string | null;
  /** 0-1 while downloading; null when the server sends no content-length. */
  progress: number | null;
  error: string | null;
  /** True once a check has completed, so the UI can distinguish "never checked". */
  checked: boolean;
}

export const updater = $state<UpdateState>({
  status: "idle",
  version: null,
  currentVersion: null,
  notes: null,
  progress: null,
  error: null,
  checked: false,
});

/** Held between check and install so the user can decide in between. */
let pending: Update | null = null;

export async function loadCurrentVersion() {
  try {
    updater.currentVersion = await getVersion();
  } catch {
    // Non-fatal: only used for display.
  }
}

/**
 * The plugin returns this when the endpoint has no usable release JSON — a 404
 * is not treated as "no update", it throws. That is the normal state before the
 * first release is published, so it reads as up-to-date rather than an error.
 */
const NO_RELEASE = "Could not fetch a valid release JSON from the remote";

/**
 * Ask the endpoint whether a newer build exists.
 *
 * `silent` is for the automatic check on launch: a missing endpoint, an offline
 * machine, or a dev build with no updater artifacts must not throw an error
 * banner at someone who never asked for a check.
 */
export async function checkForUpdate(silent = false): Promise<boolean> {
  if (updater.status === "checking" || updater.status === "downloading") return false;
  updater.status = "checking";
  updater.error = null;
  try {
    const update = await check();
    updater.checked = true;
    if (update) {
      pending = update;
      updater.version = update.version;
      updater.currentVersion = update.currentVersion ?? updater.currentVersion;
      updater.notes = update.body ?? null;
      updater.status = "available";
      return true;
    }
    pending = null;
    updater.status = "uptodate";
    return false;
  } catch (e) {
    pending = null;
    updater.checked = true;
    const message = e instanceof Error ? e.message : String(e);
    if (message.includes(NO_RELEASE)) {
      updater.status = "uptodate";
      return false;
    }
    if (silent) {
      // Leave the UI as it was; surface only in the console.
      console.warn("[deskwork] update check failed:", e);
      updater.status = "idle";
      return false;
    }
    updater.status = "error";
    updater.error = message;
    return false;
  }
}

/** Download and install the pending update, then report readiness to relaunch. */
export async function installUpdate(): Promise<boolean> {
  if (!pending || updater.status === "downloading") return false;
  updater.status = "downloading";
  updater.progress = null;
  updater.error = null;

  let total = 0;
  let received = 0;
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? 0;
        updater.progress = total > 0 ? 0 : null;
      } else if (event.event === "Progress") {
        received += event.data.chunkLength;
        if (total > 0) updater.progress = Math.min(received / total, 1);
      } else if (event.event === "Finished") {
        updater.progress = 1;
      }
    });
    updater.status = "ready";
    return true;
  } catch (e) {
    updater.status = "error";
    updater.error = e instanceof Error ? e.message : String(e);
    return false;
  }
}

/** Restart into the freshly installed version. */
export async function restartApp() {
  await relaunch();
}
