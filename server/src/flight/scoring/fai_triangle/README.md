# FAI Triangle Scoring

## Acknowledgements

This implementation owes a substantial debt to two earlier works, and the
algorithms here are in large part inspired by them:

- **Momtchil Momtchev**, author of [`igc-xc-score`](https://github.com/mmomtchev/igc-xc-score)
  — the JavaScript branch-and-bound scorer whose design (range-triple
  states, bounding-box upper bounds, lazy closest-pair search) is mirrored
  closely in this module.
- **Ondřej Palkovský**, author of "Paragliding Competition Tracklog
  Optimization" — the foundational treatment of the geometric pruning ideas that
  make exact triangle search affordable on long tracks.

Their work is the reason this scorer exists in a form we could understand,
re-implement, and extend. Sincere thanks and respect to both authors.

## Two Models

There are two scoring models in active use for "FAI triangles", and they are
not the same thing. We currently implement only the second.

### Pure FAI (Sporting Code Section 7D)

The FAI's own definition (records and badges). The pilot's flight is judged
against three **declared turn points** A, B, C — coordinates on a map, not fixes
from the track. Each turn point is the centre of a 400 m cylinder; the
start/finish is the centre of another 400 m cylinder, and "closed course" means
the track passed through the start/finish cylinder twice (once at the beginning,
once at the end). The flight qualifies if the track entered all three cylinders
in the right order.

The credited distance is the length of the **shortest open polyline** that
touches each cylinder — start-edge of A, edge of B, edge of C, finish-edge of A.
Tangent geometry; no part of the path inside a cylinder counts. Each of the
three triangle legs must be at least 28% of that polyline length. There is no
closure penalty in the formula and no multiplier — the score is just the
polyline length.

In an OLC-style league we are not the records committee, so we are not bound to
declared coordinates: we may pick A, B, C ourselves _after_ the flight, placing
each cylinder so that the right fix sits inside it (one intermediate fix inside
B and C; two fixes — one prefix, one suffix — inside A). Then we score the
resulting polyline.

We do not implement this model yet.

### OLC (and XContest / Leonardo)

What `xcontest.org` and most national leagues actually use, and what this module
computes. Three fixes A, B, C are picked from the track in time order. Each leg
of the triangle must be at least 28% of the perimeter (same shape rule as FAI).
The closure gap is measured as the smallest distance between any prefix fix (<=
A) and any suffix fix (>= C); expressed as a fraction of the perimeter, it
gates eligibility and is subtracted from the score:

- **Open FAI triangle** — closure gap ≤ 20% of perimeter, multiplier 1.4 (in xContest).
- **Closed FAI triangle** — closure gap ≤ 5% of perimeter, multiplier 1.6 (in xContest).

The score in both cases is `(perimeter - closure_gap) * multiplier`. The two
tiers are how XContest distinguishes a barely-closed triangle from a
neatly-closed one without making it pass-or-fail.

OLC (Online Contest, the German `onlinecontest.org` league that Leonardo's PHP
scorer descends from) uses the same shape rule, the same closure-gap definition,
and the same `(perimeter - closure_gap) * 1.4` formula for FAI triangles. The
difference is that OLC / Leonardo only recognise the open / 1.4 tier; there is
no closed / 1.6 upgrade in their FAI triangle category. For our purposes
"OLC-style FAI triangle" and "XContest open FAI triangle" are the same scoring
object.

## `igc-xc-score` FAI Variants

The Node library [igc-xc-score](https://github.com/mmomtchev/igc-xc-score) keeps 
XContest, plain FAI, and FAI-with-cylinders as different rule configurations 
over the same branch-and-bound triangle search. All three FAI-triangle variants 
choose three chronological fixes A, B, C, enforce the 28% minimum-side rule, and 
use the same `boundTriangle` / `scoreTriangle` machinery. The rule config 
changes the closure rule and the post-processing:

- **`XContest/fai`** accepts a closure gap up to 20% of the perimeter.
- **`FAI/fai`** accepts a fixed 0.8 km closure gap.
- **`FAI-Cylinders/fai`** first applies the same 0.8 km closure rule, then
  runs `adjustFAICylinders` while scoring each concrete candidate.

The cylinder variant uses `0.4 km` as a cylinder radius. For a triangle, the
post step moves each selected turnpoint outward from the triangle centre line,
then recomputes **each** leg as the adjusted **centre-to-centre** distance minus
two cylinder radii. In other words, candidates are compared by their
cylinder-adjusted score, not by the raw point-to-point perimeter.

This is still not the full FAI tangent/contact geometry. Given declared
cylinders, the credited distance should be the shortest polyline that touches
the observation zones in order; for a triangle, the contact point chosen on one
cylinder affects both adjacent legs. Upstream does not solve that final
one-off geometry either. A stricter implementation could still use the same
candidate search, then compute the returned distance with the tangent formula
for the selected A, B, C.

## The Problem (OLC\xContest rules)

An XContest FAI triangle asks a tighter pilot-facing question than free
distance: from the recorded fixes, can we find three turnpoints that form a
triangle whose sides are roughly balanced, and where the pilot returned close
enough to the first turnpoint to "close" it? If yes, the perimeter is rewarded
with a bigger multiplier than free distance.

The scorer chooses three fixes in time order:

```text
turnpoint 1 -> turnpoint 2 -> turnpoint 3   (and back near turnpoint 1)
```

A valid FAI triangle has two extra constraints on top of "three points in
time order":

1. **Shape.** Each side must be at least 28% of the perimeter. A long thin
   triangle does not qualify; a roughly equilateral one does.
2. **Closure.** The flight must come back near the first turnpoint. The minimum
   gap between any prefix fix (`<= turnpoint 1`) and any suffix fix (`>=
   turnpoint 3`) must be at most 20% (or 5% for closed one) of the perimeter.

If both hold, the score is:

```text
points = (perimeter - closure_gap) * 1.4
```

The 1.4 multiplier (1.6 for closed one) is what makes triangles attractive — a
100 km triangle beats a 100 km free-distance route.

```text
                tp2
               /   \
              /     \
             /       \
            /         \
          tp1 ------- tp3
           \           /
            \         /
    before tp1       after tp3
             └-------┘
            closure gap
```

The hard part — same as free distance — is that a track may contain thousands of
fixes. Trying every ordered triple is too expensive. O(n^3) plus a closure check
that is itself O(n^2) in the worst case.

Even a few thousand fixes would explode into more work than we can do
interactively. The real search problem is therefore:

1. Preserve the exact rule shape: three chronological turnpoints, side-ratio
   constraint, closure constraint, perimeter scoring with the a multiplier.
2. Avoid missing the best triangle.
3. Spend most work on parts of the track where a balanced-and-closed triangle
   could plausibly live, instead of every raw triple.
4. Be cheap on common tracks while staying bounded on adversarial ones.

Several facts make the problem tractable:

- Two of the three constraints (side ratio, closure) cap the perimeter from
  above, given just bounding boxes around groups of candidate fixes.
- Closure depends only on the prefix `[start .. tp1]` and the suffix
  `[tp3 .. end]`, not on `tp2`. The closest pair between those two halves can
  be computed once per `(tp1, tp3)` pair and reused.
- Many flights have an obvious triangle and a clear "no triangle here"
  region; a coarse first pass finds the obvious answer and rules out the
  rest cheaply.

## Rust Implementation

### Branch-and-Bound Over Range Triples

The search is a branch-and-bound walk over triples of fix-index ranges:

```text
state = (Range_A, Range_B, Range_C)   // disjoint sub-ranges of the track,
                                      // ordered in time
```

The root state has each of the three ranges set to the entire track. The solver
repeatedly:

1. Pops the pending state with the highest _upper bound_ on score.
2. Splits it by halving its widest range, producing two children.
3. For each child, computes a fresh upper bound. If the bound is `<=` the best
   known score, the child is pruned. Otherwise it is scored "from the centre" of
   each range and pushed back onto the heap.
4. Stops when the heap is empty (proven optimal).

### Upper Bound

For a given range triple `(A, B, C)`, the bound is computed against the
axis-aligned bounding boxes of the three ranges:

```text
+-----+              +-----+
|  A  |              |  C  |
+-----+              +-----+
       \           /
        +---------+
        |    B    |
        +---------+
```

- The maximum possible perimeter is the largest triangle whose vertices live
  one in each box.
- The minimum side cap (each side >= 28% of perimeter) translates to an
  upper bound on perimeter from the _shortest_ maximum cross-box distance.
- Once `A` and `C` no longer overlap in time, we can compute a lower bound on
  the closure gap from their bounding boxes. Since the score formula subtracts
  the closure gap before multiplying, a larger gap floor shrinks the upper bound
  on score — pruning more states early.

All three checks operate on bounding boxes in O(1) per state, which is what
makes the search affordable.

A `RangeBoxes` segment tree, built once per track, answers "bounding box of
fixes `[i..=j]`" in O(log n).

### Branching Heuristic

When splitting a state, the solver picks which of the three ranges to halve. The
default is "the widest range", but a range whose bounding box is much larger (`>
8x area`) than the current pick takes priority. This mirrors the igc-xc-score
heuristic: splitting the geometrically biggest range removes more of the search
space per branch than splitting the longest one.

### Closure B&B and Validity-Rectangle Cache

The closure gap is the most expensive piece of the bound: "minimum Haversine
distance between any prefix fix (≤ tp1) and any suffix fix (≥ tp3)", which is
O(n²) naively.

`ClosurePairs` solves this with its own inner B&B. Each closure query `(a, c)` —
"best pair with Q in `[0..a]` and W in `[c..n-1]`" — recursively splits the
prefix and suffix ranges, pruning sub-problems whose Haversine lower bound
already exceeds the running best. The lower bound for a prefix/suffix range pair
is computed from their bounding boxes in O(1), via the same `RangeBoxes` segment
tree used by the outer B&B.

Results are stored in a validity-rectangle cache so that overlapping queries
don't repeat work. A result `(q, w, d)` computed for query `(a₀, c₀)` is valid
for any `(a, c)` where `q ≤ a ≤ a₀` and `c₀ ≤ c ≤ w` — the new search space is a
subset of the original, and the optimal pair is still reachable. The cache is a
`BTreeMap<usize, Vec<ValidityRect>>` keyed by `a₀`:

- **Exact-hit lookup** (`lookup_cached`): `range(a..)` skips all rects with
  `a_idx < a` immediately. Any matching rect yields the exact answer — first
  hit wins, no need to scan further.
- **Cache-miss warm-up** (`best_seed_from_cache`): On a miss, scan all cached
  rects for any `(q, w)` reachable from the new search space (`q ≤ a`, `w ≥ c`).
  The best such distance seeds the inner B&B so it can prune from the start
  rather than discovering a good bound after many iterations.

### Prefilter

The prefilter stage (`probe_fai_triangle`) runs the same B&B with a relaxed
closure threshold of 25% (`FAI_CLOSURE_PREFILTER`) instead of the strict 20%.
RDP simplification can shift a real triangle's closure by a couple of metres;
without the wider band, a barely-valid candidate would be filtered out before
the strict run sees it. The trade-off is that the prefilter may surface a
triangle whose closure falls in `(20%, 25%]` — geometrically real, but one the
strict run will reject. So the probe result is a feasibility signal, not a lower
bound.

### Distance Formula

All distances — perimeter legs, closure pairs, and B&B lower bounds — use
Haversine. The choice is driven by compatibility with external platforms
(XContest, Leonardo, DHV, OLC), not by speed or accuracy requirements.

**Every distance in the scorer must use the same formula.** The B&B's pruning
relies on the lower bound being consistent with the score formula: if they
diverge, the heap's priority ordering degrades and the solver visits far more
nodes than necessary on some tracks. Mixing Haversine and FCC — even in a
single place — breaks that invariant and can cause severe performance
regressions on adversarial tracks.

### Important Tradeoffs

The scorer proves optimality when the outer B&B heap drains. The cost/accuracy
levers are:

- **`FAI_CLOSURE_PREFILTER` (25%)** widens the prefilter's acceptance band.
  Tightening it loses real triangles to RDP noise; widening it wastes
  strict-pass time on triangles the strict pass will reject.
- **`DEFAULT_MIN_SCORING_SIDE_KM` (1.4 km)** is the floor; longer flights raise
  it via `min_scoring_side_for_free_distance` so that scoring a tiny triangle on
  top of a big free-distance flight isn't worth chasing.
