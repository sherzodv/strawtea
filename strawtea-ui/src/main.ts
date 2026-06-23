import App from './App.svelte';
import { mount } from 'svelte';
import { registerSW } from 'virtual:pwa-register';
import './strawtea.css';

registerSW({ immediate: true });

const app = mount(App, {
  target: document.getElementById('app') as HTMLElement
});

export default app;
