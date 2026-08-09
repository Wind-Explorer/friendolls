<script lang="ts">
  import { onMount } from "svelte";
  import {
    commands,
    events,
    type AppMeta,
    type CursorPositions,
    type User,
  } from "$lib/bindings";

  type FriendLiveData = {
    cursor: CursorPositions | null;
    foregroundApp: AppMeta | null;
  };

  let friends: User[] = [];
  let liveData = new Map<string, FriendLiveData>();
  let error = "";

  const emptyLiveData = (): FriendLiveData => ({
    cursor: null,
    foregroundApp: null,
  });

  function applyFriends(next: User[]) {
    const friendIds = new Set(next.map((friend) => friend.id));
    friends = next;
    liveData = new Map(
      [...liveData].filter(([friendId]) => friendIds.has(friendId)),
    );
  }

  function updateCursor(friendId: string, cursor: CursorPositions) {
    liveData = new Map(liveData).set(friendId, {
      ...(liveData.get(friendId) ?? emptyLiveData()),
      cursor,
    });
  }

  function updateForegroundApp(friendId: string, foregroundApp: AppMeta) {
    liveData = new Map(liveData).set(friendId, {
      ...(liveData.get(friendId) ?? emptyLiveData()),
      foregroundApp,
    });
  }

  function initials(name: string) {
    return name.trim().slice(0, 1).toUpperCase() || "?";
  }

  function compactId(id: string) {
    return id.length > 16 ? `${id.slice(0, 8)}…${id.slice(-6)}` : id;
  }

  function cursorStyle(cursor: CursorPositions) {
    const x = Math.min(1, Math.max(0, cursor.mapped.x)) * 100;
    const y = Math.min(1, Math.max(0, cursor.mapped.y)) * 100;
    return `--cursor-x: ${x}%; --cursor-y: ${y}%;`;
  }

  onMount(() => {
    let disposed = false;
    let stopListeners: (() => void)[] = [];

    async function initialize() {
      const listeners = await Promise.all([
        events.friendsChanged.listen((event) => {
          if (!disposed) applyFriends(event.payload.friends);
        }),
        events.friendCursorPositionChanged.listen((event) => {
          if (!disposed) {
            updateCursor(event.payload.friendId, event.payload.positions);
          }
        }),
        events.friendForegroundAppChanged.listen((event) => {
          if (!disposed) {
            updateForegroundApp(event.payload.friendId, event.payload.meta);
          }
        }),
      ]);

      if (disposed) {
        listeners.forEach((stop) => stop());
        return;
      }
      stopListeners = listeners;

      try {
        const snapshot = await commands.listFriends();
        if (!disposed) applyFriends(snapshot);
      } catch (err) {
        if (!disposed) error = String(err);
      }
    }

    initialize();

    return () => {
      disposed = true;
      stopListeners.forEach((stop) => stop());
    };
  });
</script>

<section class="signal-console" aria-labelledby="friend-signals-title">
  <header>
    <div>
      <p class="eyebrow">Mutual connections</p>
      <h1 id="friend-signals-title">Friend signals</h1>
    </div>
    <span class="live-indicator"><i></i> Live</span>
  </header>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {:else if friends.length === 0}
    <div class="empty-state">
      <span aria-hidden="true">⌁</span>
      <p>Add a friend to watch their live activity arrive here.</p>
    </div>
  {:else}
    <ul>
      {#each friends as friend (friend.id)}
        {@const data = liveData.get(friend.id) ?? emptyLiveData()}
        <li>
          <div class="identity">
            <span class="avatar" aria-hidden="true">{initials(friend.displayName)}</span>
            <div>
              <h2>{friend.displayName}</h2>
              <code title={friend.id}>{compactId(friend.id)}</code>
            </div>
          </div>

          <div class="signals">
            <div class="app-signal">
              {#if data.foregroundApp}
                {#if data.foregroundApp.ico}
                  <img
                    src={`data:image/png;base64,${data.foregroundApp.ico}`}
                    alt=""
                    width="36"
                    height="36"
                  />
                {:else}
                  <span class="app-placeholder" aria-hidden="true">◆</span>
                {/if}
                <div>
                  <small>Foreground</small>
                  <strong>
                    {data.foregroundApp.local ??
                      data.foregroundApp.unlocal ??
                      "Unknown app"}
                  </strong>
                </div>
              {:else}
                <span class="app-placeholder muted" aria-hidden="true">◇</span>
                <div>
                  <small>Foreground</small>
                  <span class="waiting">Waiting for activity</span>
                </div>
              {/if}
            </div>

            <div class="cursor-signal">
              <div class="cursor-label">
                <small>Cursor</small>
                {#if data.cursor}
                  <span>
                    {Math.round(data.cursor.mapped.x * 100)} ·
                    {Math.round(data.cursor.mapped.y * 100)}
                  </span>
                {:else}
                  <span>— · —</span>
                {/if}
              </div>
              <div
                class:active={data.cursor !== null}
                class="cursor-field"
                style={data.cursor ? cursorStyle(data.cursor) : undefined}
                aria-label={data.cursor
                  ? `Cursor at ${Math.round(data.cursor.mapped.x * 100)} percent horizontal and ${Math.round(data.cursor.mapped.y * 100)} percent vertical`
                  : "Waiting for cursor position"}
              >
                <i></i>
              </div>
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .signal-console {
    --ink: #172019;
    --muted: #69736b;
    --paper: #f5f3e9;
    --line: #c8c8b9;
    --signal: #c9ff3d;
    --hot: #ff6846;
    box-sizing: border-box;
    min-height: 100%;
    padding: 1.15rem;
    overflow: hidden;
    color: var(--ink);
    font-family: "Avenir Next Condensed", "DIN Alternate", sans-serif;
    background-color: var(--paper);
    border: 1px solid var(--ink);
    box-shadow: 6px 6px 0 var(--ink);
  }

  header,
  .identity,
  .signals,
  .app-signal,
  .cursor-label {
    display: flex;
    align-items: center;
  }

  header {
    justify-content: space-between;
    gap: 1rem;
    padding-bottom: 0.9rem;
    border-bottom: 1px solid var(--ink);
  }

  h1,
  h2,
  p {
    margin: 0;
  }

  h1 {
    font-size: clamp(1.5rem, 3vw, 2.1rem);
    line-height: 0.95;
    letter-spacing: -0.04em;
  }

  .eyebrow,
  small,
  .live-indicator,
  code {
    font-family: Menlo, Monaco, monospace;
  }

  .eyebrow {
    margin-bottom: 0.25rem;
    color: var(--muted);
    font-size: 0.63rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .live-indicator {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.35rem 0.55rem;
    font-size: 0.65rem;
    text-transform: uppercase;
    border: 1px solid var(--ink);
    background: var(--signal);
  }

  .live-indicator i {
    width: 0.45rem;
    height: 0.45rem;
    background: var(--ink);
    border-radius: 50%;
    animation: pulse 1.8s ease-in-out infinite;
  }

  ul {
    display: grid;
    gap: 0.75rem;
    margin: 1rem 0 0;
    padding: 0;
    list-style: none;
  }

  li {
    padding: 0.8rem;
    background: rgba(255, 255, 255, 0.42);
    border: 1px solid var(--line);
    animation: enter 280ms ease-out both;
  }

  .identity {
    gap: 0.65rem;
    margin-bottom: 0.8rem;
  }

  .avatar {
    display: grid;
    width: 2.25rem;
    height: 2.25rem;
    place-items: center;
    flex: 0 0 auto;
    font-weight: 800;
    background: var(--hot);
    border: 1px solid var(--ink);
  }

  h2 {
    font-size: 1rem;
    line-height: 1.05;
  }

  code {
    color: var(--muted);
    font-size: 0.62rem;
  }

  .signals {
    align-items: stretch;
    gap: 0.65rem;
  }

  .app-signal,
  .cursor-signal {
    min-width: 0;
    flex: 1 1 0;
  }

  .app-signal {
    gap: 0.55rem;
  }

  .app-signal img,
  .app-placeholder {
    width: 2.25rem;
    height: 2.25rem;
    flex: 0 0 auto;
    border: 1px solid var(--ink);
  }

  .app-signal img {
    object-fit: contain;
    background: white;
  }

  .app-placeholder {
    display: grid;
    place-items: center;
    background: var(--signal);
  }

  .app-placeholder.muted {
    color: var(--muted);
    background: transparent;
    border-color: var(--line);
  }

  small {
    display: block;
    color: var(--muted);
    font-size: 0.58rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  strong,
  .waiting {
    display: block;
    max-width: 10rem;
    overflow: hidden;
    font-size: 0.75rem;
    line-height: 1.2;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .waiting {
    color: var(--muted);
  }

  .cursor-label {
    justify-content: space-between;
    margin-bottom: 0.25rem;
    color: var(--muted);
    font-family: Menlo, Monaco, monospace;
    font-size: 0.58rem;
  }

  .cursor-field {
    position: relative;
    height: 2rem;
    overflow: hidden;
    background-image: linear-gradient(var(--line) 1px, transparent 1px),
      linear-gradient(90deg, var(--line) 1px, transparent 1px);
    background-size: 25% 50%;
    border: 1px solid var(--line);
  }

  .cursor-field i {
    position: absolute;
    top: var(--cursor-y, 50%);
    left: var(--cursor-x, 50%);
    width: 0.45rem;
    height: 0.45rem;
    opacity: 0;
    background: var(--hot);
    border: 1px solid var(--ink);
    border-radius: 50%;
    transform: translate(-50%, -50%);
    transition: top 180ms linear, left 180ms linear;
  }

  .cursor-field.active i {
    opacity: 1;
  }

  .empty-state,
  .error {
    margin-top: 1rem;
    padding: 1.2rem;
    text-align: center;
    border: 1px dashed var(--line);
  }

  .empty-state span {
    display: block;
    margin-bottom: 0.35rem;
    font-size: 1.5rem;
  }

  .empty-state p,
  .error {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .error {
    color: #9a2410;
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
      transform: scale(0.75);
    }
  }

  @keyframes enter {
    from {
      opacity: 0;
      transform: translateY(5px);
    }
  }

  @media (max-width: 540px) {
    .signals {
      flex-direction: column;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .live-indicator i,
    li {
      animation: none;
    }

    .cursor-field i {
      transition: none;
    }
  }
</style>
