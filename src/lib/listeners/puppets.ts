import { writable } from "svelte/store";
import { commands, events, type PuppetState } from "$lib/bindings";

export const puppetStates = writable<PuppetState[]>([]);
export const puppetStatesListenerError = writable("");

export async function initPuppetStatesListener() {
  let receivedEvent = false;
  const unlisten = await events.puppetStatesChanged.listen((event) => {
    receivedEvent = true;
    puppetStates.set(event.payload.puppets);
  });

  try {
    const snapshot = await commands.listPuppetStates();
    // A newly created scene must not overwrite a newer event with its startup snapshot.
    if (!receivedEvent) puppetStates.set(snapshot);
  } catch (error) {
    puppetStatesListenerError.set(String(error));
  }

  return unlisten;
}
