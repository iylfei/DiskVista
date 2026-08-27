import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';
export default defineConfig({root:fileURLToPath(new URL('.',import.meta.url)),plugins:[react()],server:{port:1420,strictPort:true,host:'127.0.0.1'},build:{target:'es2021',sourcemap:false},clearScreen:false});
