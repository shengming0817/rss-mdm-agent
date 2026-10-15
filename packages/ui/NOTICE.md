# Source notice

UI layout, message rendering, composer, split math and design tokens are derived
from prmonitor revision `4dcc87264ad740da6559824e0a8b04a1c2914d4b`.
The project owner explicitly authorized extraction and MIT licensing in the C05
implementation discussion on 2026-09-09. The source tree at that revision did not
contain a LICENSE; this notice records the new authorization, not a historical
license claim. Existing source attributions are preserved where present.

The file-by-file transformation record is `docs/reference/ui-extraction.md` in
rss-mdm-agent. Vue is an external MIT-licensed peer dependency, not vendored into
this package. Its own copyright and license remain applicable.

The UI consumes Reka UI 2.10.5 (MIT), markdown-it 15.0.2 (MIT), and
Lucide Vue 1.49.0 (ISC) as external dependencies. Their licenses and attribution
remain applicable; their source is not copied into RSS-owned implementations.

## Local Reka UI corrections

The pinned Reka UI 2.10.5 dependency is patched through pnpm's locked patch
mechanism (`patches/reka-ui@2.10.5.patch`). The patch clears the ToastRootImpl
close timer on unmount and reconciles completed DateField/TimeField segment
edits with the caller's controlled value after an update event. Source, ESM
and CommonJS outputs receive the same corrections. TreeRoot also honors supplied
controlled selections (including explicit empty selection), and PopoverContent
honors a caller-supplied accessible name before its default trigger name.
The accessibility hiding hook refreshes its target subtree after live DOM
relocation, pauses under nested modals, and disconnects its observer on disposal. Existing focus, pause and
incomplete-segment behavior stays owned by Reka; no parallel implementation
is shipped. Component regression tests cover rejection/reset and early disposal.
