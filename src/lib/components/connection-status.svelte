<script lang="ts">
  import { onMount } from "svelte";
  import { commands, events, type ConnectionStatus } from "$lib/bindings";

  let statuses: ConnectionStatus[] = [];
  let error = "";

  onMount(() => {
    const unlisten = events.networkStatusChanged.listen((event) => {
      statuses = event.payload.statuses;
    });

    commands
      .listStatuses()
      .then((current) => {
        statuses = current;
      })
      .catch((err) => {
        error = String(err);
      });

    return () => {
      unlisten.then((stop) => stop());
    };
  });
</script>

<section>
  <h1>Connections</h1>

  {#if error}
    <p aria-live="polite">{error}</p>
  {:else if statuses.length === 0}
    <p>No remote connections configured.</p>
  {:else}
    <ul aria-live="polite">
      {#each statuses as status (status.remoteId)}
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
