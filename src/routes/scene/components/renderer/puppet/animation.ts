import * as THREE from "three";
import type { PuppetRig } from ".";

export class PuppetAnimation {
  private readonly smoothedGroundPosition = new THREE.Vector3();
  private readonly movementDelta = new THREE.Vector3();
  private hasSmoothedPosition = false;

  constructor(private readonly rig: PuppetRig) {}

  update(
    targetGroundPosition: THREE.Vector3,
    isMoving: boolean,
    deltaSeconds: number,
    elapsedSeconds: number,
  ) {
    if (!this.hasSmoothedPosition) {
      this.smoothedGroundPosition.copy(targetGroundPosition);
      this.rig.root.position.copy(targetGroundPosition);
      this.hasSmoothedPosition = true;
    }

    this.movementDelta
      .copy(targetGroundPosition)
      .sub(this.smoothedGroundPosition);

    if (this.movementDelta.lengthSq() <= 0.01) {
      this.pause();
      this.rig.root.position.copy(targetGroundPosition);
      return;
    }

    const alpha = 1 - Math.exp(-12 * deltaSeconds);
    this.smoothedGroundPosition.lerp(targetGroundPosition, alpha);
    this.rig.root.position.copy(this.smoothedGroundPosition);

    const targetRotationY = Math.atan2(
      this.movementDelta.x,
      this.movementDelta.z,
    );
    this.rig.root.rotation.y = this.dampAngle(
      this.rig.root.rotation.y,
      targetRotationY,
      14,
      deltaSeconds,
    );

    if (isMoving) {
      this.animateWalkCycle(elapsedSeconds);
    } else {
      this.pause();
    }
  }

  pause() {
    this.rig.leftLeg.rotation.x *= 0.8;
    this.rig.rightLeg.rotation.x *= 0.8;
    this.rig.leftArm.rotation.x *= 0.8;
    this.rig.rightArm.rotation.x *= 0.8;
  }

  private dampAngle(
    currentRadians: number,
    targetRadians: number,
    smoothingSpeed: number,
    deltaSeconds: number,
  ) {
    const deltaRadians = Math.atan2(
      Math.sin(targetRadians - currentRadians),
      Math.cos(targetRadians - currentRadians),
    );
    const alpha = 1 - Math.exp(-smoothingSpeed * deltaSeconds);
    return currentRadians + deltaRadians * alpha;
  }

  private animateWalkCycle(elapsedSeconds: number) {
    const swing = Math.sin(elapsedSeconds * 8) * 0.5;
    this.rig.leftLeg.rotation.x = swing * 2;
    this.rig.rightLeg.rotation.x = -swing * 2;
    this.rig.leftArm.rotation.x = -swing * 2;
    this.rig.rightArm.rotation.x = swing * 2;
  }
}
