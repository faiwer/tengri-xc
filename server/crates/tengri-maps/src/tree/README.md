# `tree` — single-file tile tree container

A `.tengri-dem` (or future `.tengri-*`) archive is one file: a fixed header,
then a row of fixed-size **block envelopes**, then a packed tile-data section,
then a footer magic. The viewer fetches one envelope to learn where every tile
in that block lives, optionally gets a few neighbour blocks for free, and then
range-reads tile payloads out of the data section.

## Block envelope

Every block gets exactly one **16 KiB envelope** at a known offset (`HEADER_LEN
+ block_id × BLOCK_SIZE`). Layout inside the envelope:

```
+----+--------+----------------------+----------+--------------+ ... +-----------+
| m  | len_s  | self_payload (len_s) | len_e_0  | extra_0      |     | zero pad  |
| 1B | 2B LE  |                      | 2B LE    | (len_e_0 B)  |     |           |
+----+--------+----------------------+----------+--------------+ ... +-----------+
```

- `m` — mode byte. Bit `i` set means extra `i` is appended; the bits are read in
  order, so extras are stored in mode-bit order without identifiers.
- `self_payload` — zstd-compressed **size-stream** for this block, with the
  4-byte zstd frame magic stripped (the reader prepends it back). The
  size-stream is `(tile_count, base_offset, size_0, …)` encoded as varints, plus
  an "anchor reuse" code for runs of same-length tiles.
- `extra_i` — one neighbour block's `self_payload`, stored verbatim. The reader
  can decode it on its own — same format, no per-extra metadata needed.
- Everything past the last extra is zero-pad up to `BLOCK_SIZE`.

Mode-bit order (and thus extra order):

| bit | extra |
| --- | --- |
| 0 | parent block (zoom-1) |
| 1 | sibling, horizontal |
| 2 | sibling, vertical |
| 3 | sibling, diagonal |
| 4 | cousin, horizontal (parent's sibling-h) |
| 5 | cousin, vertical |
| 6 | cousin, diagonal |

Why bundle neighbours: a viewer that just decoded block B is very likely about
to need its parent (zoom-out), one of its three siblings (pan), or a cousin (pan
into a neighbouring parent). Packing those into B's leftover bytes turns
"panning across a block boundary" from a fresh round trip into a hit on bytes
already in the client's cache.

The envelope size is **16 KiB** so a viewer pulls a whole envelope in one round
trip on a warm HTTPS connection: it matches one TLS 1.3 record (RFC 8446 §5.1)
and one HTTP/2 DATA frame, so neither layer fragments the response.

### How blocks are filled

Two phases:

1. **DFS export** writes each block's `self_payload` into its envelope (mode =
   0, no extras yet). The size-stream and the tile-data section are produced
   together, so each tile lands in the data section at the offset its
   size-stream advertises.
2. **Pack-extras end-pass** walks every block, looks at the in-RAM `len_self`
   table for its 7 candidate neighbours, picks every neighbour whose `2 +
   len_self_neighbour` fits the remaining headroom, reads those bytes back from
   the already-written envelopes, and rewrites the envelope with `mode != 0`.
   Per block the worst case is 9 × 16 KiB of I/O — sub-second on anything
   modern.

The packer is greedy in mode-bit order, so when not everything fits the closest
neighbours (parent, then siblings) win first.

## What it looks like in the wild

Profile of an arbitrary z0–z11 DEM archive (~15 GB on disk, 1,371
blocks):

- **Self payload sits well under the cap.** Max `len_self` = 8.4 KiB out of the
  16,381-byte budget (≈51 %); p50 = 1.9 KiB. The 16 KiB envelope is comfortable
  for the size-stream alone — even doubling the zoom range stays under the cap.
- **Mean envelope occupancy: 67.7 %.** p50 used = 13.9 KiB; p99 used = 16.36
  KiB; the tightest envelope left **2 free bytes**. The packer is filling
  envelopes until one more extra would overflow.
- **Extras packed: 6,263 across 1,371 blocks (4.57 / block).** 37.9 % of blocks
  pack the full 7 extras; only 0.1 % pack none.
- **Per-extra-kind hit rate** (% of blocks that include the kind): parent 98.9
  %, siblings 61–72 %, cousins 49–55 %. The closer the neighbour, the more often
  it fits.
- **Zero-pad cost: ~7 MiB across the whole archive (~0.05 %).** Negligible
  against the tile-data section, which is the other 99.9 % of the file.

Bottom line: 16 KiB is the right ceiling for the self payload (generous
headroom), and the extras packer is working the rest of the budget hard. Re-run
the profile any time with `server/leo/_tmp/analyze_archive.py
<archive.tengri-dem>`.
