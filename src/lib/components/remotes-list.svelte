<script lang="ts">
  import { onMount } from "svelte";
  import { commands, events, type Remote, type RemoteInput } from "$lib/bindings";

  let remotes: Remote[] = [];
  let editingId: string | null = null;
  let error = "";

  onMount(() => {
    const unlisten = events.remotesChanged.listen((event) => {
      remotes = event.payload.remotes;
    });

    commands.listRemotes().catch((err) => {
      error = String(err);
    });

    return () => {
      unlisten.then((stop) => stop());
    };
  });

  function remoteInput(form: HTMLFormElement): RemoteInput {
    const data = new FormData(form);
    const name = String(data.get("name") ?? "").trim();
    const port = String(data.get("port") ?? "").trim();

    return {
      address: String(data.get("address") ?? "").trim(),
      name: name || null,
      port: port ? Number(port) : null,
    };
  }

  async function createRemote(event: SubmitEvent) {
    const form = event.currentTarget as HTMLFormElement;
    error = "";

    try {
      await commands.createRemote(remoteInput(form));
      form.reset();
    } catch (err) {
      error = String(err);
    }
  }

  async function updateRemote(event: SubmitEvent, id: string) {
    const form = event.currentTarget as HTMLFormElement;
    error = "";

    try {
      await commands.updateRemote(id, remoteInput(form));
      editingId = null;
    } catch (err) {
      error = String(err);
    }
  }

  async function deleteRemote(id: string) {
    error = "";

    try {
      await commands.deleteRemote(id);
      if (editingId === id) {
        editingId = null;
      }
    } catch (err) {
      error = String(err);
    }
  }
</script>

<section>
  <h1>Remotes</h1>

  <form onsubmit={createRemote}>
    <p>
      <label>
        Address
        <input name="address" required />
      </label>
    </p>

    <p>
      <label>
        Name (optional)
        <input name="name" />
      </label>
    </p>

    <p>
      <label>
        Port (optional)
        <input name="port" type="number" min="1" max="65535" inputmode="numeric" />
      </label>
    </p>

    <button type="submit">Create remote</button>
  </form>

  {#if error}
    <p aria-live="polite">{error}</p>
  {/if}

  {#if remotes.length === 0}
    <p>No remotes saved.</p>
  {:else}
    <ul>
      {#each remotes as remote (remote.id)}
        <li>
          {#if editingId === remote.id}
            <form onsubmit={(event) => updateRemote(event, remote.id)}>
              <p>
                <label>
                  Address
                  <input name="address" value={remote.address} required />
                </label>
              </p>

              <p>
                <label>
                  Name (optional)
                  <input name="name" value={remote.name ?? ""} />
                </label>
              </p>

              <p>
                <label>
                  Port (optional)
                  <input
                    name="port"
                    type="number"
                    min="1"
                    max="65535"
                    inputmode="numeric"
                    value={remote.port ?? ""}
                  />
                </label>
              </p>

              <button type="submit">Save</button>
              <button type="button" onclick={() => (editingId = null)}>Cancel</button>
            </form>
          {:else}
            <p>
              <strong>{remote.name ?? remote.address}</strong>
              {#if remote.name}
                <span> — {remote.address}</span>
              {/if}
              {#if remote.port !== null}
                <span>:{remote.port}</span>
              {/if}
            </p>
            <p>ID: {remote.id}</p>
            <button type="button" onclick={() => (editingId = remote.id)}>Edit</button>
            <button type="button" onclick={() => deleteRemote(remote.id)}>Delete</button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>
