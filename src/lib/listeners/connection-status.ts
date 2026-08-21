import { writable } from "svelte/store";
import { commands, events, type ConnectionStatus } from "$lib/bindings";

export const connectionStatuses = writable<ConnectionStatus[]>([]);
export const connectionStatusesListenerError = writable("");

export async function initConnectionStatusesListener() {
  const unlisten = await events.networkStatusChanged.listen((event) => {
    connectionStatuses.set(event.payload.statuses);
  });

  try {
    connectionStatuses.set(await commands.listStatuses());
  } catch (error) {
    connectionStatusesListenerError.set(String(error));
  }

  return unlisten;
}
