import { resolveSkinSource } from "$lib/skins";
import * as THREE from "three";
import type { PuppetRig } from ".";

type Region = {
  x: number;
  y: number;
  width: number;
  height: number;
  flipX?: boolean;
};
type Faces = [Region, Region, Region, Region, Region, Region];
type SkinPart = {
  mesh: THREE.Mesh<THREE.BoxGeometry, THREE.Material | THREE.Material[]>;
  faces: Faces;
};

const ATLAS_SIZE = 64;
const region = (
  x: number,
  y: number,
  width: number,
  height: number,
  flipX = false,
): Region => ({ x, y, width, height, flipX });
const faces = (
  right: Region,
  left: Region,
  top: Region,
  bottom: Region,
  front: Region,
  back: Region,
): Faces => [right, left, top, bottom, front, back];

const HEAD = faces(
  region(0, 8, 8, 8, true),
  region(16, 8, 8, 8, true),
  region(8, 0, 8, 8),
  region(16, 0, 8, 8),
  region(8, 8, 8, 8),
  region(24, 8, 8, 8),
);
const BODY = faces(
  region(16, 20, 4, 12, true),
  region(28, 20, 4, 12, true),
  region(20, 16, 8, 4),
  region(28, 16, 8, 4),
  region(20, 20, 8, 12),
  region(32, 20, 8, 12),
);
const RIGHT_ARM = faces(
  region(40, 20, 4, 12, true),
  region(48, 20, 4, 12, true),
  region(44, 16, 4, 4),
  region(48, 16, 4, 4),
  region(44, 20, 4, 12),
  region(52, 20, 4, 12),
);
const LEFT_ARM = faces(
  region(32, 52, 4, 12, true),
  region(40, 52, 4, 12, true),
  region(36, 48, 4, 4),
  region(40, 48, 4, 4),
  region(36, 52, 4, 12),
  region(44, 52, 4, 12),
);
const RIGHT_LEG = faces(
  region(0, 20, 4, 12, true),
  region(8, 20, 4, 12, true),
  region(4, 16, 4, 4),
  region(8, 16, 4, 4),
  region(4, 20, 4, 12),
  region(12, 20, 4, 12),
);
const LEFT_LEG = faces(
  region(16, 52, 4, 12, true),
  region(24, 52, 4, 12, true),
  region(20, 48, 4, 4),
  region(24, 48, 4, 4),
  region(20, 52, 4, 12),
  region(28, 52, 4, 12),
);

function applyUvs(geometry: THREE.BoxGeometry, regions: Faces) {
  const uv = geometry.attributes.uv;
  regions.forEach((item, faceIndex) => {
    const offset = faceIndex * 4;
    const regionLeft = item.x / ATLAS_SIZE;
    const regionRight = (item.x + item.width) / ATLAS_SIZE;
    const left = item.flipX ? regionRight : regionLeft;
    const right = item.flipX ? regionLeft : regionRight;
    const top = 1 - item.y / ATLAS_SIZE;
    const bottom = 1 - (item.y + item.height) / ATLAS_SIZE;
    uv.setXY(offset, left, top);
    uv.setXY(offset + 1, right, top);
    uv.setXY(offset + 2, left, bottom);
    uv.setXY(offset + 3, right, bottom);
  });
  uv.needsUpdate = true;
}

function loadTexture(url: string) {
  return new Promise<THREE.Texture>((resolve, reject) => {
    new THREE.TextureLoader().load(
      url,
      (texture) => {
        texture.colorSpace = THREE.SRGBColorSpace;
        texture.magFilter = THREE.NearestFilter;
        texture.minFilter = THREE.NearestFilter;
        texture.generateMipmaps = false;
        resolve(texture);
      },
      undefined,
      reject,
    );
  });
}

export class PuppetSkin {
  private readonly material: THREE.MeshStandardMaterial;
  private texture: THREE.Texture | null = null;
  private requestedHash: string | null | undefined;
  private requestId = 0;

  constructor(
    private readonly userId: string,
    rig: PuppetRig,
  ) {
    this.material = new THREE.MeshStandardMaterial({
      color: "white",
      transparent: true,
      alphaTest: 0.5,
    });
    const parts: SkinPart[] = [
      { mesh: rig.head, faces: HEAD },
      { mesh: rig.body, faces: BODY },
      { mesh: rig.leftArmMesh, faces: LEFT_ARM },
      { mesh: rig.rightArmMesh, faces: RIGHT_ARM },
      { mesh: rig.leftLegMesh, faces: LEFT_LEG },
      { mesh: rig.rightLegMesh, faces: RIGHT_LEG },
    ];
    for (const part of parts) {
      part.mesh.material = this.material;
      applyUvs(part.mesh.geometry, part.faces);
    }
  }

  update(skinHash: string | null) {
    if (skinHash === this.requestedHash) return;
    this.requestedHash = skinHash;
    const requestId = ++this.requestId;

    resolveSkinSource(this.userId, skinHash)
      .then(async (source) => {
        if (requestId !== this.requestId) return;
        const texture = await loadTexture(source);
        if (requestId !== this.requestId) {
          texture.dispose();
          return;
        }
        this.texture?.dispose();
        this.texture = texture;
        this.material.map = texture;
        this.material.needsUpdate = true;
      })
      .catch((error) =>
        console.error(`Failed to load skin for ${this.userId}`, error),
      );
  }

  dispose() {
    this.requestId++;
    this.texture?.dispose();
    this.material.dispose();
  }
}
