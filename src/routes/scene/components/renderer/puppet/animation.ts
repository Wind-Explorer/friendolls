import * as THREE from "three";
import type { PuppetRig } from ".";

const MIN_IDLE_FACING_OFFSET_DEGREES = 3;
const MAX_IDLE_FACING_OFFSET_DEGREES = 5;

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
      this.pause();
      this.rig.root.position.copy(targetGroundPosition);
      if (!isMoving) {
        this.turnTowards(this.idleTargetRotationY, deltaSeconds);
      }
      return;
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
      this.pause();
    }
  }

  pause() {
    this.rig.leftLeg.rotation.x *= 0.8;
    this.rig.rightLeg.rotation.x *= 0.8;
    this.rig.leftArm.rotation.x *= 0.8;
    this.rig.rightArm.rotation.x *= 0.8;
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
    this.rig.root.rotation.y = this.dampAngle(
      this.rig.root.rotation.y,
      targetRotationY,
      14,
      deltaSeconds,
    );
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
