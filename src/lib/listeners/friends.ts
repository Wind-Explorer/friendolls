import { writable } from "svelte/store";
import { commands, events, type User } from "$lib/bindings";

export const friends = writable<User[]>([]);
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
