<script lang="ts">
  import { commands } from "$lib/bindings";
  import { friends, friendsListenerError } from "$lib/listeners/friends";

  let error = "";

  async function createFriend(event: SubmitEvent) {
    const form = event.currentTarget as HTMLFormElement;
    const data = new FormData(form);

    error = "";

    try {
      await commands.createFriend({
        id: String(data.get("id") ?? ""),
        displayName: String(data.get("displayName") ?? ""),
      });
      form.reset();
    } catch (err) {
      error = String(err);
    }
  }

  async function deleteFriend(id: string) {
    error = "";

    try {
      await commands.deleteFriend(id);
    } catch (err) {
      error = String(err);
    }
  }
</script>

<section>
  <h1>Friends</h1>

  <form onsubmit={createFriend}>
    <p>
      <label>
        ID
        <input name="id" required />
      </label>
    </p>

    <p>
      <label>
        Display name
        <input name="displayName" required />
      </label>
    </p>

    <button type="submit">Create friend</button>
  </form>

  {#if error || $friendsListenerError}
    <p>{error || $friendsListenerError}</p>
  {/if}

  {#if $friends.length === 0}
    <p>No friends saved.</p>
  {:else}
    <ul>
      {#each $friends as friend (friend.id)}
        <li>
          <span>{friend.displayName} ({friend.id})</span>
          <button type="button" onclick={() => deleteFriend(friend.id)}>Delete</button>
        </li>
      {/each}
    </ul>
  {/if}
</section>
