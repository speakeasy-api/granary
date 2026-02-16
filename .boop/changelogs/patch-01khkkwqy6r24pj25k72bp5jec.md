### Prior art no longer includes the current project

`granary plan` searches for related prior art when creating a new project. Previously, the just-created project would appear in its own prior art results, which was confusing and unhelpful.

The `find_prior_art` function now accepts an `exclude_id` parameter and filters out the current project before returning results. This means prior art suggestions only contain genuinely distinct projects.

Includes tests covering both the exclusion logic and the edge case where the only search match is the excluded project itself.