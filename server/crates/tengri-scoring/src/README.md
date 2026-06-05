# Formulas

To measure distances, at least at the very final step where the route points have already been computed, all popular services use Haversine with R=6,371,000. It's not the fastest formula, and it's not the most accurate one either, so the reason why everyone uses it is unclear.

What do we use? It depends:

- For the most CPU-bound tasks we use the flat geometry formula (`sqrt(AC^2 + BC^2)`). It doesn't take the Earth's curvature into account. Where does that matter? The farther from the Equator, the worse the error. E.g., it can be 2x or 3x bigger/smaller in Alaska.
  - To make it more accurate, we transform the whole track (or large parts of it) from Earth coordinates to flat points.
- For moderately CPU-bound tasks we use FCC. It's quite accurate for PG/HG track distances and can be more precise than Haversine.
- At the very last step, when we calculate the distance between the solution points, we use Haversine. Why? Just to stay aligned with other popular HG/PG solutions.
