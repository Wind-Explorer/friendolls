import type { PuppetState } from "$lib/bindings";
import type { PuppetScreenBounds } from "../types";
import type { World } from "../world";
import { Puppet } from ".";

export class PuppetManager {
  private readonly puppets = new Map<string, Puppet>();

  constructor(private readonly world: World) {}

  update(
    states: readonly PuppetState[],
    frozenPuppetId: string | null,
    deltaSeconds: number,
    elapsedSeconds: number,
  ) {
    this.syncPuppets(states);

    for (const state of states) {
      this.puppets
        .get(state.id)
        ?.update(
          state,
          this.world,
          state.id === frozenPuppetId,
          deltaSeconds,
          elapsedSeconds,
        );
    }
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

  private syncPuppets(states: readonly PuppetState[]) {
    const nextPuppetIds = new Set(states.map((state) => state.id));

    for (const state of states) {
      if (this.puppets.has(state.id)) continue;

      const puppet = new Puppet(state.id);
      this.puppets.set(puppet.id, puppet);
      this.world.addObject(puppet.root);
    }

    for (const [puppetId, puppet] of this.puppets) {
      if (nextPuppetIds.has(puppetId)) continue;

      this.world.removeObject(puppet.root);
      puppet.dispose();
      this.puppets.delete(puppetId);
    }
  }
}
