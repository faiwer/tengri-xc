# Free Distance Scoring

## The Problem

Free distance asks a simple pilot-facing question: given the ordered GPS fixes
from a flight, what is the longest route the pilot can claim without needing to
close a triangle or return near the start?

The scorer chooses five fixes in time order:

```text
start -> turnpoint 1 -> turnpoint 2 -> turnpoint 3 -> finish
```

The score distance is the sum of those four legs. The scoring start is the first
chosen scoring fix, not necessarily the first recorded fix near launch. The
chosen points do not need to be evenly spaced, and they do not need to match
obvious visual corners of the track.

```text
             S
             |
             |
             a
             |
P1 ----------+---------------------- P2 ---------------------- P3
```

If `S` is the recorded launch-side start, the scorer may still choose `P1` as
the first scoring point. Spending a limited scoring point on `S` only helps when
the `S -> P1` opening leg is worth more than using that point later to capture a
larger bend. A long glide away from launch can use the launch-side point as the
first point and the farthest reachable fix as the finish. A meandering flight
can skip weak early movement and use intermediate turnpoints to collect distance
from useful bends.

The rules say "up to 3 turnpoints"; they do not explain why this exact count
won. The best practical explanations are that five total points are enough to
represent the classic XC shapes, that the same shape feeds free distance and
triangle scoring, and that a fixed small route keeps scores comparable across
flights. Computation may also be part of the reason: every extra free turnpoint
makes exact search much more expensive.

The hard part is that a track may contain thousands of fixes. Trying every
ordered 5-point route is too expensive:

```text
O(n^5)
```

Even a few thousand fixes would explode into more combinations than we can score
interactively. The real search problem is therefore:

1. Preserve the exact rule shape: five chronological points, four distance legs.
2. Avoid missing the best route when possible.
3. Spend most work near plausible route-shaping points instead of every raw fix.
4. Return quickly for common tracks, while still having a bounded fallback for
   harder shapes.

Several facts make the problem tractable:

- Free distance has no closure constraint, so route quality is driven only by
  leg length and chronological order.
- The best route tends to pass through geometric extremes of the track, not
  through thousands of nearly collinear fixes.
- A simplified version of the track is often enough to identify the rough route.
- Once a rough route is known, the exact raw-track search can be limited to
  small windows around the route points.

From a pilot perspective this matches intuition: a scorer should find the big
out-and-back bends, zigzags, and final glide endpoint, but it should not care
about every second of straight-line cruise.

## Rust Implementation

The public API lives in `mod.rs`:

- `evaluate_free_distance(track)` returns the free-distance route result.

Before scoring, `ScoringTrack` removes consecutive duplicate coordinates. The
route is mapped back to the original track indexes after scoring, so callers see
points from the input track rather than the deduped working track.

### Dynamic Programming Path

`evaluate_dp` finds the route with a dynamic-programming solver. For `m` working
track indexes, this is roughly:

```text
O(5 * m^2)
```

That is dramatically smaller than `O(n^5)`, and it is a good fit when RDP
simplification has already reduced the track to meaningful shape points.

The DP solve is exact for the working indexes it receives. The heuristic part is
choosing those indexes:

1. Build an RDP-simplified working track near a target size.
2. Run DP on that working track.
3. Keep raw indexes in windows around the current route points.
4. RDP-simplify that compacted window track and run DP again.
5. Halve the window size and repeat.
6. When the compacted raw window is small enough, run DP directly on raw indexes
   and return that route.

The final route is therefore not limited to RDP points. RDP is used to find the
rough shape cheaply; the last pass goes back to raw fixes near that shape.

### Target-Count RDP

The scorer does not use a fixed RDP tolerance ladder. It asks
`simplify_track_to_target_count` for a simplified track near
`RDP_TARGET_POINTS`, with `RDP_TARGET_SPREAD` as the allowed range.

With the current constants, the target is:

```text
500 +/- 10% = 450..550 points
```

The helper binary-searches the tolerance between `RDP_MIN_TOLERANCE_M` and
`RDP_MAX_TOLERANCE_M`.

The RDP call is capped at the upper target bound. If a tolerance would keep too
many points, RDP stops early and reports `TooMany`; this avoids spending time
building a candidate set that DP would reject as too dense anyway.

If binary search cannot land inside the target range, the helper returns the
densest complete RDP result it found under the max-point cap. That keeps the
scorer shape-based without falling back to every raw fix in normal cases.

If even that fails, the initial pass uses the whole raw track as the fallback.
It means even the largest allowed simplification was still too dense under the
cap.

### Refinement Loop

After the first DP route is found, refinement works by squeezing the raw track
around the route:

```text
raw track fixes:
  0  1  2  3  4  5  6  7  8  9  ...  n
     .----- dense noise -----.

Found solution candidates:
  P1 ------ P2 ------ P3 ------ P4 ------ P5

refinement window around route:
  [near P1] xxxxx [near P2] xxxxx [near P3] xxxxx [near P4] xxxxx [near P5]
            ^drop           ^drop           ^drop           ^drop
```

`squeeze_route` keeps a percentage window around each route point. The first
window is `REFINE_START_WINDOW_PERCENT` of the whole track length. Each
iteration halves that percentage.

If the squeezed raw window is still large, it is converted into a compact track
and simplified with the same target-count RDP settings as the first pass. The
resulting compact indexes are mapped back to original raw indexes before DP runs.

If the squeezed raw window has fewer than `REFINE_MIN_WINDOW_POINTS`, the scorer
stops simplifying and runs DP directly on those raw indexes. At that size, exact
DP is cheap and more precise than another RDP pass.

### DP Table

The DP table is indexed by:

```text
state[leg][end]
```

where:

- `leg` is how many legs have already been chosen
- `end` is the current route endpoint inside the working indexes

To fill one cell, the solver tries every earlier working index as the previous
route point:

```text
state[leg][end] =
  max over start < end:
    state[leg - 1][start] + distance(start, end)
```

Every transition moves forward in time, so chronological order is guaranteed by
construction.

### Important Tradeoffs

The scorer is exact over each working index set, but the working index set is
chosen heuristically. RDP keeps the track shape and removes dense noise; route
windows focus the later passes near the current best route.

The constants in `constants.rs` control the cost/accuracy balance:

- `RDP_TARGET_POINTS` is the preferred working-track size before DP. Because RDP
  keeps shape points, this is more accurate than choosing every Nth point.
- `REFINE_START_WINDOW_PERCENT` decides how much raw track is kept around the
  first provisional route. Each iteration halves the value.
- `REFINE_MIN_WINDOW_POINTS` decides when the scorer stops simplifying and runs
  DP directly on raw indexes.

When changing the algorithm, compare both distance changes and runtime. A faster
candidate pass is not useful if it systematically misses route-shaping fixes,
and a more exact search is not useful if common flights become slow enough to
hurt ingestion or audit runs.
