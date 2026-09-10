import { writable } from "svelte/store";
import { commands, events } from "$lib/bindings";

export const onlineFriendIds = writable<Set<string>>(new Set());
export const friendStatusesListenerError = writable("");

export async function initFriendStatusesListener() {
  const apply = (friendIds: string[]) => {
    onlineFriendIds.set(new Set(friendIds));
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
