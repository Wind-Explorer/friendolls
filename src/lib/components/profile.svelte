<script lang="ts">
  import { onMount } from "svelte";
  import { commands, events, type User } from "$lib/bindings";

  let profile: User | null = null;
  let displayName = "";
  let error = "";

  onMount(() => {
    let stop: (() => void) | undefined;
    let disposed = false;

    async function initialize() {
      const unlisten = await events.profileChanged.listen((event) => {
        profile = event.payload.profile;
        displayName = event.payload.profile.displayName;
      });

      if (disposed) {
        unlisten();
        return;
      }

      stop = unlisten;
      await commands.getProfile();
    }

    initialize().catch((err) => {
      error = String(err);
    });

    return () => {
      disposed = true;
      stop?.();
    };
  });

  async function updateProfile(event: SubmitEvent) {
    event.preventDefault();
    error = "";

    try {
      await commands.updateProfile(displayName);
    } catch (err) {
      error = String(err);
    }
  }
</script>

<section>
  <h1>Profile</h1>

  {#if profile}
    <p>Public key: {profile.id}</p>
  {:else}
    <p>Loading profile...</p>
  {/if}

  <form onsubmit={updateProfile}>
    <label>
      Display name
      <input name="displayName" bind:value={displayName} required />
    </label>
    <button type="submit">Save profile</button>
  </form>

  {#if error}
    <p>{error}</p>
  {/if}
</section>
