### Fix: unblock returns tasks to \`todo\` and clears stale claims

Previously, \`unblock_task\` checked \`started_at\` to decide whether to return a task to \`in_progress\` or \`todo\`. Since agents set \`started_at\` when they begin work, unblocking always returned tasks to \`in_progress\` — even when the block was due to external factors (e.g., missing files from concurrent projects) and the task needed to re-enter the scheduling queue.

Additionally, unblock didn't clear \`claim_owner\` or \`claim_lease_expires_at\`. This meant the \`task.next\` SQLite trigger's claim guard would suppress the event even after manually cycling the task through \`draft → todo\`. The net effect was that unblocked tasks became invisible to the worker daemon — no \`task.next\` event fired, so no worker would pick them up.

**What changed:**
- \`unblock_task\` now always transitions to \`todo\`, regardless of \`started_at\`
- Stale claim fields (\`claim_owner\`, \`claim_claimed_at\`, \`claim_lease_expires_at\`) are cleared on unblock
- The \`task.next\` trigger now fires correctly when tasks are unblocked