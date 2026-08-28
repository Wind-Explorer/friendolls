import type { PuppetState } from "$lib/bindings";
import * as THREE from "three";
import type { World } from "../world";
import { PuppetAnimation } from "./animation";

export type PuppetRig = {
  root: THREE.Group;
  leftArm: THREE.Group;
  rightArm: THREE.Group;
  leftLeg: THREE.Group;
  rightLeg: THREE.Group;
};

export class Puppet {
  readonly root: THREE.Group;

  private readonly rig: PuppetRig;
  private readonly animation: PuppetAnimation;
  private readonly targetGroundPosition = new THREE.Vector3();

  constructor(readonly id: string) {
    this.rig = this.createRig();
    this.root = this.rig.root;
    this.animation = new PuppetAnimation(this.rig);
  }

  update(
    state: PuppetState,
    world: World,
    frozen: boolean,
    deltaSeconds: number,
    elapsedSeconds: number,
  ) {
    if (frozen) {
      this.animation.pause();
      return;
    }

    world.normalizedPointToGroundPoint(
      state.position.x,
      state.position.y,
      this.targetGroundPosition,
    );
    this.animation.update(
      this.targetGroundPosition,
      state.isMoving,
      deltaSeconds,
      elapsedSeconds,
    );
  }

  dispose() {
    const disposedMaterials = new Set<THREE.Material>();

    this.root.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return;

      object.geometry.dispose();
      const materials = Array.isArray(object.material)
        ? object.material
        : [object.material];
      for (const material of materials) {
        if (disposedMaterials.has(material)) continue;
        material.dispose();
        disposedMaterials.add(material);
      }
    });
  }

  private createRig(): PuppetRig {
    const root = new THREE.Group();
    const material = new THREE.MeshStandardMaterial({
      color: this.colorFromId(),
    });

    const body = new THREE.Mesh(new THREE.BoxGeometry(16, 24, 8), material);
    body.position.y = 36;

    const head = new THREE.Mesh(new THREE.BoxGeometry(16, 16, 16), material);
    head.position.y = 56;

    const leftArm = this.createLimbPivot(-12, 48, 8, 24, 8, material);
    const rightArm = this.createLimbPivot(12, 48, 8, 24, 8, material);
    const leftLeg = this.createLimbPivot(-4, 24, 8, 24, 8, material);
    const rightLeg = this.createLimbPivot(4, 24, 8, 24, 8, material);

    root.add(body, head, leftArm, rightArm, leftLeg, rightLeg);

    return { root, leftArm, rightArm, leftLeg, rightLeg };
  }

  private createLimbPivot(
    x: number,
    y: number,
    width: number,
    height: number,
    depth: number,
    material: THREE.Material,
  ) {
    const pivot = new THREE.Group();
    pivot.position.set(x, y, 0);

    const limb = new THREE.Mesh(
      new THREE.BoxGeometry(width, height, depth),
      material,
    );
    limb.position.y = -height / 2;
    pivot.add(limb);

    return pivot;
  }

  private colorFromId() {
    let hash = 0;
    for (const character of this.id) {
      hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
    }

    return new THREE.Color().setHSL((hash % 360) / 360, 0.55, 0.62);
  }
}
