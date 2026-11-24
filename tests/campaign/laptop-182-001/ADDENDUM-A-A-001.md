# Prospective A/A instrumentation clarification

This addendum applies to the native instrumentation qualification for the actual-laptop protocol, before any native A/A observations or comparative campaign data are collected. It preserves the original `PROTOCOL.md` bytes, SHA-256 `67f14e04a8750ab9ed8e7b1ec6c3b02f22dff8a7a51774444f263672044587fe`.

The ordinary main-thread observer runs on its fixed five-second calendar in both A/A conditions. OFF disables only the deliberately assigned extra counter/resource sample scheduled one millisecond after that slot's dispatch. ON adds that sample. Both conditions retain the same dispatch publication, wake, marker, receipt and release mechanics. The only observer threads are the main sampler and the existing invasive barrier thread; all observer CPU remains accounted.

Here dispatch is the explicit worker marker immediately before the unchanged stock `runner.run_slot` call. It is distinct from the stock measured start and the child process creation boundary. Retain each assigned condition, dispatch, original measured start/end, requested target, actual wake, complete native read span and receipt-flush span. Retain late wakes, misses and non-overlaps under their original assignments. Never shift a target, drop a row, rerun a pair or force overlap in response to an observation.

The main sampler processes the earliest due target first; an ordinary five-second endpoint wins an exact tie with an extra probe target. Record original due times and actual service times. The ordinary sampler calendar is never shifted to a lane boundary. An OFF lane may overlap an ordinary five-second sample; OFF does not mean wholly unobserved.

This A/A contrast characterizes the incremental extra-probe and recording overhead conditional on the ordinary sampler. It does not measure the total observer cost, pure native API latency, or an uninstrumented product's performance. Report actual exposure and dispatch-to-measurement lag; nominal one-millisecond scheduling does not establish overlap.

Keep the original 60 matched relay pairs and 12 matched long pairs, deterministic seed/domain and assigned order. Long pairs use the pinned trusted guard and original two-second/30-second stimulus. Qualify the current candidate's deadline separately if the trusted guard is a different artifact. Preserve original workload, console contexts, endpoints, stream/survivor oracles, containment and cleanup.

Complete every assignment. If the assignments and tail settlement finish before 65 seconds from the first during-phase anchor, keep the owned worker idle until that fixed timestamp, within the unchanged finite overall deadline. This predetermined minimum supports the required complete five-second windows; it is not extended in response to observed measurements or gate failures. Keep the existing native window, CPU, clock, coverage, phase-transition, barrier and replay checks. Preserve any failure or incomplete attempt without rescore.

Before execution, both independent Sol reviewers must approve these same addendum bytes and the exact qualification implementation. Bind this addendum and its reviews to the qualification recipe and the campaign's frozen inputs. No native qualification or campaign approval follows merely from this clarification.
