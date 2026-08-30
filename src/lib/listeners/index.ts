import { initConnectionStatusesListener } from "./connection-status";
import { initFriendStatusesListener } from "./friend-statuses";
import { initFriendsListener } from "./friends";
import { initInteractionListener } from "./interactions";
import { initLiveMetadataListeners } from "./live-metadata";
import { initProfileListener } from "./profile";
import { initPuppetStatesListener } from "./puppets";
import { initRemotesListener } from "./remotes";
import { initSceneConfigurationListener } from "./scene-configuration";

type Unlisten = () => void;

async function initPresenceListeners(): Promise<Unlisten> {
  const unlistenLiveMetadata = await initLiveMetadataListeners();
  try {
    const unlistenFriendStatuses = await initFriendStatusesListener();
    return () => {
      unlistenFriendStatuses();
      unlistenLiveMetadata();
    };
  } catch (error) {
    unlistenLiveMetadata();
    throw error;
  }
}

export function initAppListeners(): Unlisten {
  let disposed = false;
  let unlisteners: Unlisten[] = [];
  Promise.allSettled([
    initFriendsListener(),
    initRemotesListener(),
    initProfileListener(),
    initConnectionStatusesListener(),
    initPresenceListeners(),
    initInteractionListener(),
    initPuppetStatesListener(),
    initSceneConfigurationListener(),
  ])
    .then((results) => {
      const listeners = results.flatMap((result) =>
        result.status === "fulfilled" ? [result.value] : [],
      );

      results.forEach((result) => {
        if (result.status === "rejected") {
          console.error("failed to initialize app listener", result.reason);
        }
      });

      if (disposed) {
        listeners.forEach((unlisten) => unlisten());
      } else {
        unlisteners = listeners;
      }
    });

  return () => {
    disposed = true;
    unlisteners.forEach((unlisten) => unlisten());
    unlisteners = [];
  };
}
