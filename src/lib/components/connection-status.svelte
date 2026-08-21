<script lang="ts">
  import {
    connectionStatuses,
    connectionStatusesListenerError,
  } from "$lib/listeners/connection-status";
</script>

<section>
  <h1>Connections</h1>

  {#if $connectionStatusesListenerError}
    <p aria-live="polite">{$connectionStatusesListenerError}</p>
  {:else if $connectionStatuses.length === 0}
    <p>No remote connections configured.</p>
  {:else}
    <ul aria-live="polite">
      {#each $connectionStatuses as status (status.remoteId)}
        <li>
          <strong>{status.name ?? status.address}</strong>: {status.state}
          {#if status.name}
            <span> — {status.address}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>
