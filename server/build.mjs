import esbuild from 'esbuild';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const srcDir = path.join(__dirname, 'src');
const outDir = path.join(__dirname, 'dist');

console.log(`Building from ${srcDir} to ${outDir}...`);

try {
  const result = esbuild.buildSync({
    entryPoints: [path.join(srcDir, 'app.ts')],
    bundle: true,
    platform: 'node',
    target: 'node20',
    outdir: outDir,
    sourcemap: true,
    packages: 'external',  // This marks all node_modules as external
    format: 'cjs',
    logLevel: 'info',
  });
  console.log('✅ Build complete!');
} catch (e) {
  console.error('Build failed:', e.message);
  process.exit(1);
}
