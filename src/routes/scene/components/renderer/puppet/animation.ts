import * as THREE from "three";
import type { PuppetRig } from "./visual";

const MIN_IDLE_FACING_OFFSET_DEGREES = 3;
const MAX_IDLE_FACING_OFFSET_DEGREES = 5;
const ANGLE_EPSILON = 0.001;
const LIMB_DAMPING = -60 * Math.log(0.8);

export class PuppetAnimation {
  private readonly smoothedGroundPosition = new THREE.Vector3();
  private readonly movementDelta = new THREE.Vector3();
  private readonly idleRotationY: number;
  private idleTargetRotationY: number;
  private hasSmoothedPosition = false;
  private wasMoving = false;

  constructor(private readonly rig: PuppetRig) {
    this.idleRotationY = rig.root.rotation.y;
    this.idleTargetRotationY = this.idleRotationY;
  }

  /** Return true until motion and pose settling no longer need another frame. */
  update(
    targetGroundPosition: THREE.Vector3,
    isMoving: boolean,
    deltaSeconds: number,
    elapsedSeconds: number,
  ) {
    this.updateIdleTarget(isMoving);

    if (!this.hasSmoothedPosition) {
      this.smoothedGroundPosition.copy(targetGroundPosition);
      this.rig.root.position.copy(targetGroundPosition);
      this.hasSmoothedPosition = true;
    }

    this.movementDelta
      .copy(targetGroundPosition)
      .sub(this.smoothedGroundPosition);

    if (this.movementDelta.lengthSq() <= 0.01) {
      const relaxing = this.pause(deltaSeconds);
      this.smoothedGroundPosition.copy(targetGroundPosition);
      this.rig.root.position.copy(targetGroundPosition);
      const turning =
        !isMoving && this.turnTowards(this.idleTargetRotationY, deltaSeconds);
      return relaxing || turning;
    }

    const alpha = 1 - Math.exp(-12 * deltaSeconds);
    this.smoothedGroundPosition.lerp(targetGroundPosition, alpha);
    this.rig.root.position.copy(this.smoothedGroundPosition);

    if (isMoving) {
      this.turnTowards(
        Math.atan2(this.movementDelta.x, this.movementDelta.z),
        deltaSeconds,
      );
      this.animateWalkCycle(elapsedSeconds);
    } else {
      this.turnTowards(this.idleTargetRotationY, deltaSeconds);
      this.pause(deltaSeconds);
    }
    return true;
  }

  pause(deltaSeconds: number) {
    let active = false;
    const damping = Math.exp(-LIMB_DAMPING * deltaSeconds);
    for (const limb of [
      this.rig.leftLeg,
      this.rig.rightLeg,
      this.rig.leftArm,
      this.rig.rightArm,
    ]) {
      limb.rotation.x *= damping;
      if (Math.abs(limb.rotation.x) <= ANGLE_EPSILON) limb.rotation.x = 0;
      else active = true;
    }
    return active;
  }

  walk(elapsedSeconds: number) {
    this.animateWalkCycle(elapsedSeconds);
  }

  private updateIdleTarget(isMoving: boolean) {
    if (isMoving) {
      this.wasMoving = true;
      return;
    }
    if (!this.wasMoving) return;

    const magnitudeDegrees = THREE.MathUtils.lerp(
      MIN_IDLE_FACING_OFFSET_DEGREES,
      MAX_IDLE_FACING_OFFSET_DEGREES,
      Math.random(),
    );
    const direction = Math.random() < 0.5 ? -1 : 1;
    this.idleTargetRotationY =
      this.idleRotationY +
      THREE.MathUtils.degToRad(magnitudeDegrees * direction);
    this.wasMoving = false;
  }

  private turnTowards(targetRotationY: number, deltaSeconds: number) {
    const current = this.rig.root.rotation.y;
    const delta = Math.atan2(
      Math.sin(targetRotationY - current),
      Math.cos(targetRotationY - current),
    );
    if (Math.abs(delta) <= ANGLE_EPSILON) {
      this.rig.root.rotation.y = targetRotationY;
      return false;
    }
    this.rig.root.rotation.y =
      current + delta * (1 - Math.exp(-14 * deltaSeconds));
    return true;
  }

  private animateWalkCycle(elapsedSeconds: number) {
    const swing = Math.sin(elapsedSeconds * 8) * 0.5;
    this.rig.leftLeg.rotation.x = swing * 2;
    this.rig.rightLeg.rotation.x = -swing * 2;
    this.rig.leftArm.rotation.x = -swing * 2;
    this.rig.rightArm.rotation.x = swing * 2;
  }
}
