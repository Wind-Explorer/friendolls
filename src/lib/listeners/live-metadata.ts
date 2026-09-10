import { writable } from "svelte/store";
import {
  commands,
  events,
  type Activity,
  type CursorPositions,
} from "$lib/bindings";

export type LiveMetadata = {
  localId: string;
  cursorPositions: Partial<Record<string, CursorPositions>>;
  activities: Map<string, Map<string, Activity>>;
};

export const liveMetadata = writable<LiveMetadata>({
  localId: "",
  cursorPositions: {},
  activities: new Map(),
});
export const liveMetadataListenerError = writable("");

export async function initLiveMetadataListeners() {
  let localId = "";
  let initializing = true;
  let pendingCursorPositions: LiveMetadata["cursorPositions"] | null = null;
  const pendingActivities = new Map<string, Map<string, Activity>>();

  const activityMap = (
    activities: Partial<Record<string, Activity>>,
  ): Map<string, Activity> =>
    new Map(
      Object.entries(activities).filter(
        (entry): entry is [string, Activity] => entry[1] !== undefined,
      ),
    );

  const replaceActivities = (
    userId: string,
    userActivities: Map<string, Activity>,
  ) => {
    liveMetadata.update((current) => {
      const activities = new Map(current.activities);
      if (userActivities.size > 0) activities.set(userId, userActivities);
      else activities.delete(userId);
      return { ...current, activities };
    });
  };

  const applyPendingUpdates = () => {
    const cursorPositions = pendingCursorPositions;
    if (cursorPositions) {
      liveMetadata.update((current) => ({ ...current, cursorPositions }));
      pendingCursorPositions = null;
    }
    pendingActivities.forEach((activities, userId) => {
      replaceActivities(userId, activities);
    });
    pendingActivities.clear();
  };

  const subscriptions = await Promise.allSettled([
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
    events.activitiesChanged.listen((event) => {
      const activities = activityMap(event.payload.activities);
      if (initializing) {
        pendingActivities.set(event.payload.userId, activities);
      } else {
        replaceActivities(event.payload.userId, activities);
      }
    }),
  ]);
  const unlisteners = subscriptions.flatMap((subscription) =>
    subscription.status === "fulfilled" ? [subscription.value] : [],
  );
  for (const subscription of subscriptions) {
    if (subscription.status === "rejected") {
      unlisteners.forEach((unlisten) => unlisten());
      throw subscription.reason;
    }
  }

  try {
    const [snapshot] = await Promise.all([
      commands.listLiveData(),
      commands.getPublicKey().then((resolvedLocalId) => {
        localId = resolvedLocalId;
      }),
    ]);
    const activities = new Map<string, Map<string, Activity>>();
    Object.entries(snapshot.activities).forEach(
      ([userId, sourceActivities]) => {
        if (!sourceActivities) return;
        const userActivities = activityMap(sourceActivities);
        if (userActivities.size > 0) {
          activities.set(userId, userActivities);
        }
      },
    );
    liveMetadata.set({
      localId,
      cursorPositions: snapshot.cursorPositions,
      activities,
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
