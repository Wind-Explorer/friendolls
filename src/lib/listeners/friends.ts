import { writable } from "svelte/store";
import { commands, events, type Friend } from "$lib/bindings";

export const UNRESOLVED_FRIEND_NAME = "Name pending resolution";

export function friendName(friend: Friend | undefined, fallback: string) {
  return friend?.displayName ?? (friend ? UNRESOLVED_FRIEND_NAME : fallback);
}

export const friends = writable<Friend[]>([]);
export const friendsListenerError = writable("");

export async function initFriendsListener() {
  const unlisten = await events.friendsChanged.listen((event) => {
    friends.set(event.payload.friends);
  });

  try {
    friends.set(await commands.listFriends());
  } catch (error) {
    friendsListenerError.set(String(error));
  }

  return unlisten;
}
