import { startMockProvider } from "./mock-provider.mjs";

const mock = await startMockProvider();
process.stdout.write(`${mock.endpoint}\n`);
process.stdin.resume();
process.on("SIGTERM", async () => { await mock.close(); process.exit(0); });
