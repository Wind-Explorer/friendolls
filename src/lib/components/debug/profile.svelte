<script lang="ts">
  import { commands } from "$lib/bindings";
  import { profile, profileListenerError } from "$lib/listeners/profile";

  let displayName = "";
  let error = "";

  $: displayName = $profile?.displayName ?? "";

  async function updateProfile(event: SubmitEvent) {
    event.preventDefault();
    error = "";

    try {
      await commands.updateProfile(displayName, null);
    } catch (err) {
      error = String(err);
    }
  }
</script>

<section>
  <h1>Profile</h1>

  {#if $profile}
    <p>Public key: {$profile.id}</p>
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

  {#if error || $profileListenerError}
    <p>{error || $profileListenerError}</p>
  {/if}
</section>
