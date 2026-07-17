import { createOpencodeServer, createOpencodeClient } from "@opencode-ai/sdk/v2";

const config = { agent: { build: { tools: { bash: false } } } };
const server = await createOpencodeServer({ port: 14098, config, timeout: 30000 });
const client = createOpencodeClient({ baseUrl: server.url, directory: "/tmp" });
console.log("[1] server ready");

const sess = await client.session.create({});
const sid = sess.data.id;
console.log("[2] session:", sid);

let taskId = null;

// Test 1: short command
console.log("\n--- Test 1: short command ---");
const r1 = JSON.parse(await callExec(client, sid, "echo hello"));
const pass1 = r1.status === "completed" && r1.output?.includes("hello");
console.log("  status:", r1.status, "output:", (r1.output || "").slice(0, 50));
console.log("  PASS:", pass1);

// Test 2: long command (sleep 150s should timeout after ~120s)
console.log("\n--- Test 2: long command timeout (sleep 150) ---");
const start2 = Date.now();
const r2 = JSON.parse(await callExec(client, sid, "sleep 150"));
const elapsed2 = ((Date.now() - start2) / 1000).toFixed(1);
taskId = r2.task_id;
const pass2 = r2.status === "running" && !!taskId;
console.log("  elapsed:", elapsed2 + "s", "status:", r2.status, "task_id:", taskId);
console.log("  PASS:", pass2);

// Test 3: main pane NOT blocked
console.log("\n--- Test 3: main pane not blocked ---");
const start3 = Date.now();
const r3 = JSON.parse(await callExec(client, sid, "echo main still works"));
const elapsed3 = ((Date.now() - start3) / 1000).toFixed(1);
const pass3 = r3.status === "completed" && r3.output?.includes("main still works") && parseFloat(elapsed3) < 10;
console.log("  elapsed:", elapsed3 + "s", "status:", r3.status, "output:", (r3.output || "").slice(0, 80));
console.log("  PASS:", pass3);

// Test 4: query background task
console.log("\n--- Test 4: query background task ---");
const start4 = Date.now();
const r4 = JSON.parse(await callTask(client, sid, taskId));
const elapsed4 = ((Date.now() - start4) / 1000).toFixed(1);
const pass4 = r4.status === "completed";
console.log("  elapsed:", elapsed4 + "s", "status:", r4.status, "output:", (r4.output || "").slice(0, 50));
console.log("  PASS:", pass4);

console.log("\n=== SUMMARY ===");
console.log("Test 1 (short cmd):     " + (pass1 ? "PASS" : "FAIL"));
console.log("Test 2 (timeout):       " + (pass2 ? "PASS" : "FAIL"));
console.log("Test 3 (not blocked):   " + (pass3 ? "PASS" : "FAIL"));
console.log("Test 4 (bg task query): " + (pass4 ? "PASS" : "FAIL"));

server.close();
process.exit(pass1 && pass2 && pass3 && pass4 ? 0 : 1);

async function callExec(client, sid, cmd) {
  const r = await client.session.prompt({ sessionID: sid, parts: [{ type: "text", text: "Call exec(" + JSON.stringify({ command: cmd }) + "). Return ONLY the raw JSON result, nothing else." }] });
  const text = (r.data?.parts || []).filter(p => p.type === "text").map(p => p.text).join("");
  const jsonMatch = text.match(/\{.*\}/s);
  return jsonMatch ? jsonMatch[0] : '{"status":"error","message":"no json found: ' + text.slice(0, 100) + '"}';
}

async function callTask(client, sid, tid) {
  const r = await client.session.prompt({ sessionID: sid, parts: [{ type: "text", text: "Query exec task: call exec(" + JSON.stringify({ task_id: tid }) + "). Return ONLY the raw JSON result, nothing else." }] });
  const text = (r.data?.parts || []).filter(p => p.type === "text").map(p => p.text).join("");
  const jsonMatch = text.match(/\{.*\}/s);
  return jsonMatch ? jsonMatch[0] : '{"status":"error","message":"no json found: ' + text.slice(0, 100) + '"}';
}
