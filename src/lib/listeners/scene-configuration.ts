import { writable } from "svelte/store";
import {
  commands,
  events,
  type SceneConfiguration,
} from "$lib/bindings";

export const sceneConfiguration = writable<SceneConfiguration>({
  puppetScale: 1,
  puppetOpacity: 1,
  puppetMovementMode: "free",
  hideLocalPuppetWhenAlone: false,
});
export const sceneConfigurationListenerError = writable("");

export async function initSceneConfigurationListener() {
  const unlisten = await events.sceneConfigurationChanged.listen((event) => {
    sceneConfiguration.set(event.payload.configuration);
  });

  try {
    sceneConfiguration.set(await commands.getSceneConfiguration());
  } catch (error) {
    sceneConfigurationListenerError.set(String(error));
  }

  return unlisten;
}
