import { copyFileSync } from 'node:fs';
import { join } from 'node:path';

const root = new URL('.', import.meta.url);
const source = new URL('./source/landing.html', root);
const target = new URL('./public/index.html', root);
copyFileSync(source, target);
console.log('Homepage copied to public/index.html');