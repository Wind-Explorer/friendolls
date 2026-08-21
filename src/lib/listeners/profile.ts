import { writable } from "svelte/store";
import { commands, events, type User } from "$lib/bindings";

export const profile = writable<User | null>(null);
export const profileListenerError = writable("");

export async function initProfileListener() {
  const unlisten = await events.profileChanged.listen((event) => {
    profile.set(event.payload.profile);
  });

  try {
    profile.set(await commands.getProfile());
  } catch (error) {
    profileListenerError.set(String(error));
  }

  return unlisten;
}
