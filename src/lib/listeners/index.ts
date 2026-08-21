import { initConnectionStatusesListener } from "./connection-status";
import { initFriendsListener } from "./friends";
import { initLiveMetadataListeners } from "./live-metadata";
import { initProfileListener } from "./profile";
import { initRemotesListener } from "./remotes";

type Unlisten = () => void;

export function initAppListeners(): Unlisten {
  let disposed = false;
  let unlisteners: Unlisten[] = [];

  Promise.allSettled([
    initFriendsListener(),
    initRemotesListener(),
    initProfileListener(),
    initConnectionStatusesListener(),
    initLiveMetadataListeners(),
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
