import { writable } from 'svelte/store';

type RouteState = {
  path: string;
};

function currentPath(): string {
  return window.location.pathname || '/';
}

function createRouteStore() {
  const store = writable<RouteState>({ path: '/' });

  return {
    subscribe: store.subscribe,
    navigate(path: string) {
      window.history.pushState({}, '', path);
      store.set({ path });
    },
    sync() {
      store.set({ path: currentPath() });
    }
  };
}

export const route = createRouteStore();

export function startRouter() {
  route.sync();
  window.addEventListener('popstate', () => route.sync());
}
