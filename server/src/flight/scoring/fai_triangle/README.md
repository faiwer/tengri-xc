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
