import { writable } from "svelte/store";
import { commands, events, type Remote } from "$lib/bindings";

export const remotes = writable<Remote[]>([]);
export const remotesListenerError = writable("");

export async function initRemotesListener() {
  const unlisten = await events.remotesChanged.listen((event) => {
    remotes.set(event.payload.remotes);
  });

  try {
    remotes.set(await commands.listRemotes());
  } catch (error) {
    remotesListenerError.set(String(error));
  }

  return unlisten;
}
