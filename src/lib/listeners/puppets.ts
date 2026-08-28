import { writable } from "svelte/store";
import { commands, events, type PuppetState } from "$lib/bindings";

export const puppetStates = writable<PuppetState[]>([]);
export const puppetStatesListenerError = writable("");

export async function initPuppetStatesListener() {
  const unlisten = await events.puppetStatesChanged.listen((event) => {
    puppetStates.set(event.payload.puppets);
  });

  try {
    puppetStates.set(await commands.listPuppetStates());
  } catch (error) {
    puppetStatesListenerError.set(String(error));
  }

  return unlisten;
}
