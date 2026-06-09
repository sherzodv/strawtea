<script lang="ts">
  import { onMount } from 'svelte';
  import { LogOut } from '@lucide/svelte';
  import HomePage from './features/home/HomePage.svelte';
  import StocksPage from './features/stocks/StocksPage.svelte';
  import LoginPage from './features/auth/LoginPage.svelte';
  import { auth, initAuth, signOut } from './lib/auth';
  import { route, startRouter } from './lib/router';

  onMount(() => {
    initAuth();
    startRouter();
  });

  $: isReady = $auth.status !== 'loading';
  $: isSignedIn = $auth.status === 'signed-in';
</script>

{#if !isReady}
  <main class="shell center">
    <p class="muted">Loading</p>
  </main>
{:else if !isSignedIn}
  <LoginPage />
{:else}
  <main class="shell">
    <header class="topbar">
      <div>
        <p class="eyebrow">Strawtea</p>
        <p class="account">{$auth.user?.email}</p>
      </div>
      <button class="icon-button" type="button" aria-label="Sign out" on:click={signOut}>
        <LogOut size={20} />
      </button>
    </header>

    {#if $route.path === '/'}
      <HomePage />
    {:else if $route.path === '/stocks'}
      <StocksPage />
    {:else}
      <section class="empty-state">
        <h1>Not found</h1>
        <a href="/" on:click|preventDefault={() => route.navigate('/')}>Back home</a>
      </section>
    {/if}
  </main>
{/if}
