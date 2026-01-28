---
name: update-e2e-tests
description: Update end-to-end tests after CLI API changes. Use when public commands, flags, or output formats have been modified.
---

# Update E2E Tests

After modifying the CLI's public API:

1. Add tests to `./test-e2e.sh` following existing patterns
2. Use assertions to verify output:
   ```bash
   OUTPUT=$(granary <command> --json)
   [[ -n "$OUTPUT" ]] || { echo "FAIL: Expected output"; exit 1; }
   echo "$OUTPUT" | grep -q "expected" || { echo "FAIL: Missing expected"; exit 1; }
   ```
3. All operations MUST use `$TEST_DIR` - never the repo root
4. Run `./test-e2e.sh` to verify
