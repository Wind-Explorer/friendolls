/** One render loop for the scene. Updates return true while animation remains. */
export class SceneScheduler {
  private frameId: number | null = null;
  private previousTimestamp: number | null = null;
  private elapsedSeconds = 0;
  private suspended = false;
  private disposed = false;

  constructor(
    private readonly update: (deltaSeconds: number, elapsedSeconds: number) => boolean,
  ) {}

  invalidate = () => {
    if (this.disposed || this.suspended || this.frameId !== null) return;
    this.frameId = requestAnimationFrame(this.frame);
  };

  setSuspended(suspended: boolean) {
    this.suspended = suspended;
    if (suspended) this.cancel();
    else this.invalidate();
  }

  dispose() {
    this.disposed = true;
    this.cancel();
  }

  private cancel() {
    if (this.frameId !== null) cancelAnimationFrame(this.frameId);
    this.frameId = null;
    this.previousTimestamp = null;
  }

  private frame = (timestamp: number) => {
    this.frameId = null;
    if (this.disposed || this.suspended) return;
    // Sleep and long stalls must not advance an animation by a huge step.
    const delta = this.previousTimestamp === null
      ? 0
      : Math.min((timestamp - this.previousTimestamp) / 1000, 0.1);
    this.previousTimestamp = timestamp;
    this.elapsedSeconds += delta;
    if (this.update(delta, this.elapsedSeconds)) this.invalidate();
    if (this.frameId === null) this.previousTimestamp = null;
  };
}
