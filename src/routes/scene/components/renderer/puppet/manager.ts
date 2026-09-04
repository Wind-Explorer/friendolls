import type { PuppetState } from "$lib/bindings";
import type { PuppetScreenBounds, SceneRenderInputs } from "../types";
import type { World } from "../world";
import { Puppet } from ".";

export class PuppetManager {
  private readonly puppets = new Map<string, Puppet>();

  constructor(
    private readonly world: World,
    private readonly onChange: () => void,
  ) {}

  update(
    {
      puppets: states,
      scale,
      idleOpacity,
      selectedPuppetId,
      skinHashes,
    }: SceneRenderInputs,
    deltaSeconds: number,
    elapsedSeconds: number,
  ) {
    this.syncPuppets(states, scale);

    let active = false;
    for (const state of states) {
      const puppetActive = this.puppets
        .get(state.id)
        ?.update(
          state,
          this.world,
          state.id === selectedPuppetId,
          deltaSeconds,
          elapsedSeconds,
          skinHashes.get(state.id) ?? null,
          state.id === selectedPuppetId ? 1 : idleOpacity,
        );
      active = puppetActive || active;
    }
    return active;
  }

  screenBounds(): PuppetScreenBounds[] {
    return [...this.puppets.values()].map((puppet) => ({
      id: puppet.id,
      ...this.world.objectScreenBounds(puppet.root),
    }));
  }

  dispose() {
    for (const puppet of this.puppets.values()) {
      this.world.removeObject(puppet.root);
      puppet.dispose();
    }
    this.puppets.clear();
  }

  private syncPuppets(states: readonly PuppetState[], scale: number) {
    const nextPuppetIds = new Set(states.map((state) => state.id));

    for (const state of states) {
      if (this.puppets.has(state.id)) continue;

      const puppet = new Puppet(state.id, scale, this.onChange);
      this.puppets.set(puppet.id, puppet);
      this.world.addObject(puppet.root);
    }

    for (const puppet of this.puppets.values()) puppet.setScale(scale);

    for (const [puppetId, puppet] of this.puppets) {
      if (nextPuppetIds.has(puppetId)) continue;

      this.world.removeObject(puppet.root);
      puppet.dispose();
      this.puppets.delete(puppetId);
    }
  }
}
