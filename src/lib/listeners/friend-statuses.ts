import { writable } from "svelte/store";
import { commands, events } from "$lib/bindings";
import { removeFriendForegroundApps } from "./live-metadata";

export const onlineFriendIds = writable<Set<string>>(new Set());
export const friendStatusesListenerError = writable("");

export async function initFriendStatusesListener() {
  let current = new Set<string>();

  const apply = (friendIds: string[]) => {
    const next = new Set(friendIds);
    const wentOffline = [...current].filter((friendId) => !next.has(friendId));
    current = next;
    onlineFriendIds.set(next);

    removeFriendForegroundApps(wentOffline);
  };

  const unlisten = await events.friendStatusesChanged.listen((event) => {
    apply(event.payload.friendIds);
  });

  try {
    apply(await commands.listFriendStatuses());
  } catch (error) {
    friendStatusesListenerError.set(String(error));
  }

  return unlisten;
}
