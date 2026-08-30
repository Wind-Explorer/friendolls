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

export function retainOnlineForegroundApps(onlineFriendIds: Set<string>) {
  liveMetadata.update((metadata) => ({
    ...metadata,
    foregroundApps: new Map(
      [...metadata.foregroundApps].filter(
        ([userId]) =>
          userId === metadata.localId || onlineFriendIds.has(userId),
      ),
    ),
  }));
}

export async function initLiveMetadataListeners() {
  let localId = "";
  let initializing = true;
  let pendingCursorPositions: LiveMetadata["cursorPositions"] | null = null;
  let pendingLocalForegroundApp: AppMeta | null = null;
  const pendingFriendForegroundApps = new Map<string, AppMeta>();

  const updateForegroundApp = (userId: string, meta: AppMeta) => {
    liveMetadata.update((current) => ({
      ...current,
      foregroundApps: new Map(current.foregroundApps).set(userId, meta),
    }));
  };

  const applyPendingUpdates = () => {
    const cursorPositions = pendingCursorPositions;
    if (cursorPositions) {
      liveMetadata.update((current) => ({ ...current, cursorPositions }));
      pendingCursorPositions = null;
    }
    if (localId && pendingLocalForegroundApp) {
      updateForegroundApp(localId, pendingLocalForegroundApp);
      pendingLocalForegroundApp = null;
    }
    pendingFriendForegroundApps.forEach((meta, friendId) => {
      updateForegroundApp(friendId, meta);
    });
    pendingFriendForegroundApps.clear();
  };

  const unlisteners = await Promise.all([
    events.cursorPositionChanged.listen((event) => {
      if (initializing) {
        pendingCursorPositions = event.payload.positions;
        return;
      }
      liveMetadata.update((current) => ({
        ...current,
        cursorPositions: event.payload.positions,
      }));
    }),
    events.foregroundAppChanged.listen((event) => {
      if (!initializing && localId) {
        updateForegroundApp(localId, event.payload.meta);
      } else {
        pendingLocalForegroundApp = event.payload.meta;
      }
    }),
    events.friendForegroundAppChanged.listen((event) => {
      if (initializing) {
        pendingFriendForegroundApps.set(
          event.payload.friendId,
          event.payload.meta,
        );
      } else {
        updateForegroundApp(event.payload.friendId, event.payload.meta);
      }
    }),
  ]);

  try {
    const [snapshot] = await Promise.all([
      commands.listLiveData(),
      commands.getPublicKey().then((resolvedLocalId) => {
        localId = resolvedLocalId;
      }),
    ]);
    const foregroundApps = new Map<string, AppMeta>();
    Object.entries(snapshot.foregroundApps).forEach(([userId, meta]) => {
      if (meta) foregroundApps.set(userId, meta);
    });
    liveMetadata.set({
      localId,
      cursorPositions: snapshot.cursorPositions,
      foregroundApps,
    });
    initializing = false;
    applyPendingUpdates();
  } catch (error) {
    initializing = false;
    if (localId) {
      liveMetadata.update((current) => ({ ...current, localId }));
    }
    applyPendingUpdates();
    liveMetadataListenerError.set(String(error));
  }

  return () => unlisteners.forEach((unlisten) => unlisten());
}
