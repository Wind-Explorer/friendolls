<script lang="ts">
  import {
    liveMetadata,
    liveMetadataListenerError,
  } from "$lib/listeners/live-metadata";

  function liveUserIds() {
    return [
      ...new Set([
        $liveMetadata.localId,
        ...Object.keys($liveMetadata.cursorPositions),
        ...$liveMetadata.activities.keys(),
      ]),
    ].filter(Boolean);
  }

  function compactId(id: string) {
    return id.length > 16 ? `${id.slice(0, 8)}…${id.slice(-6)}` : id;
  }
</script>

<section>
  <h1>Live metadata</h1>

  {#if $liveMetadataListenerError}
    <p>{$liveMetadataListenerError}</p>
  {:else if liveUserIds().length === 0}
    <p>Waiting for live metadata...</p>
  {:else}
    {#each liveUserIds() as userId (userId)}
      {@const cursor = $liveMetadata.cursorPositions[userId]}
      {@const activities = $liveMetadata.activities.get(userId)}
      <article>
        <h2>{userId === $liveMetadata.localId ? "Local user" : "Remote user"}</h2>
        <p title={userId}>ID: {compactId(userId)}</p>

        <h3>Cursor</h3>
        {#if cursor}
          <p>Raw: {cursor.raw.x}, {cursor.raw.y}</p>
          <p>Mapped: {cursor.mapped.x}, {cursor.mapped.y}</p>
        {:else}
          <p>Waiting for cursor data...</p>
        {/if}

        <h3>Activities</h3>
        {#if activities}
          {#each activities as [source, activity]}
            <section>
              <h4>{source} ({activity.kind})</h4>
              {#if activity.icon}
                <img
                  src={`data:image/png;base64,${activity.icon}`}
                  alt=""
                  width="64"
                  height="64"
                />
              {/if}
              <p>Name: {activity.name ?? "Unavailable"}</p>
              <p>Details: {activity.details ?? "Unavailable"}</p>
            </section>
          {/each}
        {:else}
          <p>Waiting for activity data...</p>
        {/if}
      </article>
    {/each}
  {/if}
</section>
