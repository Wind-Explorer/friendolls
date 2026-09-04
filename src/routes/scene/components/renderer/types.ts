import type { PuppetState } from "$lib/bindings";

export type SceneRenderInputs = {
  puppets: readonly PuppetState[];
  scale: number;
  idleOpacity: number;
  selectedPuppetId: string | null;
  skinHashes: ReadonlyMap<string, string | null>;
};

export type PuppetScreenBounds = {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
};
