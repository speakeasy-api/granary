---
name: bump-plugin-version
description: Update plugin and marketplace versions after changes to .claude-plugin/. Use when skills, commands, or plugin config have been modified.
---

# Bump Plugin Version

After modifying `.claude-plugin/` files:

1. Read current version from `.claude-plugin/plugin.json`
2. Bump version (patch for fixes, minor for new skills/commands)
3. Update `version` in both:
   - `.claude-plugin/plugin.json`
   - `.claude-plugin/marketplace.json` (root `version` AND `plugins[0].version`)
4. Verify all three version fields match
