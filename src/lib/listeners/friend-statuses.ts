import { writable } from "svelte/store";
import { commands, events } from "$lib/bindings";
import { retainOnlineForegroundApps } from "./live-metadata";

export const onlineFriendIds = writable<Set<string>>(new Set());
export const friendStatusesListenerError = writable("");

export async function initFriendStatusesListener() {
  const apply = (friendIds: string[]) => {
    const next = new Set(friendIds);
    onlineFriendIds.set(next);
    retainOnlineForegroundApps(next);
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
