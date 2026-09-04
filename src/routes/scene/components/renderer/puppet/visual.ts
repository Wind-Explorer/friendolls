import * as THREE from "three";
import { PuppetAnimation } from "./animation";
import { PuppetSkin } from "./skin";

export const CAMERA_FACING_ROTATION_Y = Math.PI / 4;
export const SOUTH_WEST_ROTATION_Y = -Math.PI / 0.01;

export type PuppetAppearance = {
  skinHash: string | null;
  skinSource?: string | null;
  scale: number;
  opacity: number;
};

export type PuppetRig = {
  root: THREE.Group;
  body: THREE.Mesh<THREE.BoxGeometry, THREE.Material | THREE.Material[]>;
  head: THREE.Mesh<THREE.BoxGeometry, THREE.Material | THREE.Material[]>;
  leftArm: THREE.Group;
  leftArmMesh: THREE.Mesh<THREE.BoxGeometry, THREE.Material | THREE.Material[]>;
  rightArm: THREE.Group;
  rightArmMesh: THREE.Mesh<
    THREE.BoxGeometry,
    THREE.Material | THREE.Material[]
  >;
  leftLeg: THREE.Group;
  leftLegMesh: THREE.Mesh<THREE.BoxGeometry, THREE.Material | THREE.Material[]>;
  rightLeg: THREE.Group;
  rightLegMesh: THREE.Mesh<
    THREE.BoxGeometry,
    THREE.Material | THREE.Material[]
  >;
};

export class PuppetVisual {
  readonly root: THREE.Group;

  private readonly rig: PuppetRig;
  private readonly animation: PuppetAnimation;
  private readonly skin: PuppetSkin;

  constructor(userId: string, onChange: () => void = () => {}) {
    this.rig = createRig();
    this.root = this.rig.root;
    this.animation = new PuppetAnimation(this.rig);
    const placeholderMaterial = this.rig.body.material;
    this.skin = new PuppetSkin(userId, this.rig, onChange);
    if (!Array.isArray(placeholderMaterial)) placeholderMaterial.dispose();
  }

  setAppearance({ skinHash, skinSource, scale, opacity }: PuppetAppearance) {
    if (skinSource) this.setSkinSource(skinSource);
    else this.setSkin(skinHash);
    this.setScale(scale);
    this.setOpacity(opacity);
  }

  setSkin(skinHash: string | null) {
    this.skin.update(skinHash);
  }

  setSkinSource(source: string) {
    this.skin.updateSource(source);
  }

  setScale(scale: number) {
    if (this.root.scale.x !== scale) this.root.scale.setScalar(scale);
  }

  setOpacity(opacity: number) {
    this.skin.setOpacity(opacity);
  }

  updateMotion(
    targetGroundPosition: THREE.Vector3,
    isMoving: boolean,
    deltaSeconds: number,
    elapsedSeconds: number,
  ) {
    return this.animation.update(
      targetGroundPosition,
      isMoving,
      deltaSeconds,
      elapsedSeconds,
    );
  }

  walkInPlace(facingRotationY: number, elapsedSeconds: number) {
    this.root.rotation.y = facingRotationY;
    this.animation.walk(elapsedSeconds);
  }

  pause(deltaSeconds: number) {
    return this.animation.pause(deltaSeconds);
  }

  dispose() {
    this.skin.dispose();

    const geometries = new Set<THREE.BufferGeometry>();
    this.root.traverse((object) => {
      if (object instanceof THREE.Mesh) geometries.add(object.geometry);
    });
    for (const geometry of geometries) geometry.dispose();
  }
}

function createRig(): PuppetRig {
  const root = new THREE.Group();
  root.rotation.y = CAMERA_FACING_ROTATION_Y;
  const material = new THREE.MeshStandardMaterial();

  const body = new THREE.Mesh(new THREE.BoxGeometry(16, 24, 8), material);
  body.position.y = 36;

  const head = new THREE.Mesh(new THREE.BoxGeometry(16, 16, 16), material);
  head.position.y = 56;

  const { pivot: leftArm, mesh: leftArmMesh } = createLimbPivot(
    -12,
    48,
    8,
    24,
    8,
    material,
  );
  const { pivot: rightArm, mesh: rightArmMesh } = createLimbPivot(
    12,
    48,
    8,
    24,
    8,
    material,
  );
  const { pivot: leftLeg, mesh: leftLegMesh } = createLimbPivot(
    -4,
    24,
    8,
    24,
    8,
    material,
  );
  const { pivot: rightLeg, mesh: rightLegMesh } = createLimbPivot(
    4,
    24,
    8,
    24,
    8,
    material,
  );

  root.add(body, head, leftArm, rightArm, leftLeg, rightLeg);

  return {
    root,
    body,
    head,
    leftArm,
    leftArmMesh,
    rightArm,
    rightArmMesh,
    leftLeg,
    leftLegMesh,
    rightLeg,
    rightLegMesh,
  };
}

function createLimbPivot(
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

  return { pivot, mesh: limb };
}
