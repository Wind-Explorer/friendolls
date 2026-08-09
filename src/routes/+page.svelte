<script>
  import FriendsList from "$lib/components/friends-list.svelte";
  import FriendsLiveData from "$lib/components/friends-live-data.svelte";
  import ConnectionStatus from "$lib/components/connection-status.svelte";
  import Profile from "$lib/components/profile.svelte";
  import RemotesList from "$lib/components/remotes-list.svelte";
  import LiveMetadata from "$lib/components/live-metadata.svelte";
</script>

<main>
  <div>
    <Profile />
  </div>
  <div class="friends-area">
    <div>
      <FriendsList />
    </div>
    <FriendsLiveData />
  </div>
  <div>
    <RemotesList />
  </div>
  <div>
    <ConnectionStatus />
  </div>
  <div>
    <LiveMetadata />
  </div>
</main>

<style>
</style>
