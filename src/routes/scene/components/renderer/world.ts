import * as THREE from "three";

export class World {
  private readonly renderer: THREE.WebGLRenderer;
  private readonly scene: THREE.Scene;
  private readonly camera: THREE.OrthographicCamera;
  private readonly objectBounds = new THREE.Box3();
  private readonly projectedCorner = new THREE.Vector3();

  constructor(private readonly container: HTMLDivElement) {
    const { width, height } = this.getRenderSize();

    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setSize(width, height);

    this.camera = new THREE.OrthographicCamera(
      width / -2,
      width / 2,
      height / 2,
      height / -2,
      0.1,
      5_000,
    );
    this.camera.position.set(2_000, 1_000, 2_000);
    this.camera.lookAt(0, 0, 0);
    this.camera.updateProjectionMatrix();

    this.scene = new THREE.Scene();
    this.scene.background = null;

    const ambientLight = new THREE.AmbientLight("white", 0.8);
    const directionalLight = new THREE.DirectionalLight("white", 2);
    directionalLight.position.set(2_000, 1_500, 1_500);
    this.scene.add(ambientLight, directionalLight);

    this.container.appendChild(this.renderer.domElement);
  }

  addObject(object: THREE.Object3D) {
    this.scene.add(object);
  }

  removeObject(object: THREE.Object3D) {
    this.scene.remove(object);
  }

  normalizedPointToGroundPoint(
    normalizedX: number,
    normalizedY: number,
    target: THREE.Vector3,
  ) {
    const ndcX = normalizedX * 2 - 1;
    const ndcY = -normalizedY * 2 + 1;
    const rayOrigin = new THREE.Vector3(ndcX, ndcY, -1).unproject(this.camera);
    const rayDirection = new THREE.Vector3(0, 0, -1)
      .transformDirection(this.camera.matrixWorld)
      .normalize();
    const distanceToGround = -rayOrigin.y / rayDirection.y;

    target.copy(rayOrigin).addScaledVector(rayDirection, distanceToGround);
  }

  objectScreenBounds(object: THREE.Object3D) {
    object.updateWorldMatrix(true, true);
    this.objectBounds.setFromObject(object);

    const { min, max } = this.objectBounds;
    let minX = Number.POSITIVE_INFINITY;
    let minY = Number.POSITIVE_INFINITY;
    let maxX = Number.NEGATIVE_INFINITY;
    let maxY = Number.NEGATIVE_INFINITY;

    for (const x of [min.x, max.x]) {
      for (const y of [min.y, max.y]) {
        for (const z of [min.z, max.z]) {
          this.projectedCorner.set(x, y, z).project(this.camera);
          minX = Math.min(minX, this.projectedCorner.x);
          maxX = Math.max(maxX, this.projectedCorner.x);
          minY = Math.min(minY, this.projectedCorner.y);
          maxY = Math.max(maxY, this.projectedCorner.y);
        }
      }
    }

    const rect = this.container.getBoundingClientRect();
    const left = rect.left + ((minX + 1) / 2) * rect.width;
    const right = rect.left + ((maxX + 1) / 2) * rect.width;
    const top = rect.top + ((1 - maxY) / 2) * rect.height;
    const bottom = rect.top + ((1 - minY) / 2) * rect.height;

    return {
      x: left,
      y: top,
      width: right - left,
      height: bottom - top,
    };
  }

  render() {
    this.renderer.render(this.scene, this.camera);
  }

  resizeWorld = () => {
    const { width, height } = this.getRenderSize();

    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setSize(width, height, false);
    this.camera.left = width / -2;
    this.camera.right = width / 2;
    this.camera.top = height / 2;
    this.camera.bottom = height / -2;
    this.camera.updateProjectionMatrix();
  };

  dispose() {
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }

  private getRenderSize() {
    return {
      width: this.container.clientWidth,
      height: this.container.clientHeight,
    };
  }
}
