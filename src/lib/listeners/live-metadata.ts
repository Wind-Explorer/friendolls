import { writable } from "svelte/store";
import {
  commands,
  events,
  type AppMeta,
  type CursorPositions,
} from "$lib/bindings";

export type LiveMetadata = {
  localId: string;
  cursorPositions: Partial<Record<string, CursorPositions>>;
  foregroundApps: Map<string, AppMeta>;
};

export const liveMetadata = writable<LiveMetadata>({
  localId: "",
  cursorPositions: {},
  foregroundApps: new Map(),
});
export const liveMetadataListenerError = writable("");

export function removeFriendForegroundApps(friendIds: string[]) {
  if (friendIds.length === 0) return;
  const removed = new Set(friendIds);
  liveMetadata.update((metadata) => ({
    ...metadata,
    foregroundApps: new Map(
      [...metadata.foregroundApps].filter(([userId]) => !removed.has(userId)),
    ),
  }));
}

export async function initLiveMetadataListeners() {
  let localId = "";
  let pendingLocalForegroundApp: AppMeta | null = null;

  const updateForegroundApp = (userId: string, meta: AppMeta) => {
    liveMetadata.update((current) => ({
      ...current,
      foregroundApps: new Map(current.foregroundApps).set(userId, meta),
    }));
  };

  const unlisteners = await Promise.all([
    events.cursorPositionChanged.listen((event) => {
      liveMetadata.update((current) => ({
        ...current,
        cursorPositions: event.payload.positions,
      }));
    }),
    events.foregroundAppChanged.listen((event) => {
      if (localId) {
        updateForegroundApp(localId, event.payload.meta);
      } else {
        pendingLocalForegroundApp = event.payload.meta;
      }
    }),
    events.friendForegroundAppChanged.listen((event) => {
      updateForegroundApp(event.payload.friendId, event.payload.meta);
    }),
  ]);

  try {
    localId = await commands.getPublicKey();
    liveMetadata.update((current) => ({ ...current, localId }));

    if (pendingLocalForegroundApp) {
      updateForegroundApp(localId, pendingLocalForegroundApp);
      pendingLocalForegroundApp = null;
    }
  } catch (error) {
    liveMetadataListenerError.set(String(error));
  }

  return () => unlisteners.forEach((unlisten) => unlisten());
}
