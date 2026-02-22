### Fixed progress bars showing as fully filled or empty

Progress bars throughout the initiative and project detail screens were rendering as binary — either completely empty or completely full — instead of showing actual proportional progress. A project at 15% completion would display an entirely filled bar.

The root cause was that `FillPortion` in Iced only distributes space among sibling elements. Each progress bar had a single child container inside the track, so `FillPortion(15)` behaved identically to `FillPortion(100)` — always taking 100% of the parent.

The fix adds a second container as a sibling to split the space proportionally, with conditional logic to omit the unused portion at 0% and 100% so both edge cases render correctly.

Affected locations:
- Reusable `ProgressBar` widget
- Initiative detail screen (main progress bar and per-project mini bars)
- Project detail screen progress section
- Project card progress bar