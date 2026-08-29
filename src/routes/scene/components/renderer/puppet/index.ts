import type { PuppetState } from "$lib/bindings";
import * as THREE from "three";
import type { World } from "../world";
import { PuppetVisual } from "./visual";

export class Puppet {
  readonly root: THREE.Group;

  private readonly visual: PuppetVisual;
  private readonly targetGroundPosition = new THREE.Vector3();

  constructor(
    readonly id: string,
    scale: number,
  ) {
    this.visual = new PuppetVisual(id);
    this.root = this.visual.root;
    this.visual.setScale(scale);
    this.visual.setOpacity(1);
  }

  setScale(scale: number) {
    this.visual.setScale(scale);
  }

  update(
    state: PuppetState,
    world: World,
    frozen: boolean,
    deltaSeconds: number,
    elapsedSeconds: number,
    skinHash: string | null,
    opacity: number,
  ) {
    this.visual.setSkin(skinHash);
    this.visual.setOpacity(opacity);
    if (frozen) {
      this.visual.pause();
      return;
    }

    world.normalizedPointToGroundPoint(
      state.position.x,
      state.position.y,
      this.targetGroundPosition,
    );
    this.visual.updateMotion(
      this.targetGroundPosition,
      state.isMoving,
      deltaSeconds,
      elapsedSeconds,
    );
  }

  dispose() {
    this.visual.dispose();
  }
}
