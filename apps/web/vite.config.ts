import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import Icons from 'unplugin-icons/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [Icons({ compiler: 'svelte' }), tailwindcss(), sveltekit()],
  server: {
    proxy: {
      '/api': {
        target: process.env.DEPO_API_ORIGIN ?? 'http://127.0.0.1:3847',
        changeOrigin: true
      }
    }
  }
});
