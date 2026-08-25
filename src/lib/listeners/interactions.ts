import { writable } from "svelte/store";
import { events, type FriendInteractionReceived } from "$lib/bindings";

export const incomingInteraction = writable<FriendInteractionReceived | null>(
  null,
);

export async function initInteractionListener() {
  return events.friendInteractionReceived.listen((event) => {
    incomingInteraction.set(event.payload);
  });
}
