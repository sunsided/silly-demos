# Online hierarchical splitting: evidence rules and local repair

A reference note for `crates/stream/src/tree.rs`, where a leaf that reaches `split_at` points splits
by k-means with k = 1..8 chosen by BIC and the decision is never revisited. It covers (a) when to
split, (b) how to repair earlier decisions locally. Everything under each method is from the cited
primary source unless marked **[unverified]**. The last section holds our adaptations, each marked
**[ours]**.

---

## 1. ODAC (Rodrigues, Gama, Pedroso)

ODAC clusters **variables** (streams), not examples. A leaf holds a set of streams, every example
updates each leaf's per-pair sufficient statistics (Σa, Σb, Σa², Σb², Σab), and the dissimilarity is
`rnomc(a,b) = sqrt((1 − corr(a,b))/2)`, with range [0,1].

I read three versions. I **could not access the 2008 TKDE paper** (IEEE TKDE 20(5):615–627,
doi:10.1109/TKDE.2007.190727; Semantic Scholar marks it closed access, and ResearchGate and
academia.edu return 403). Its exact equations are **[unverified]**. River's implementation, which
cites it, is summarized at the end of this section as a secondary source.

### Rule (SIAM SDM 2006 version, "ODAC: Hierarchical Clustering of Time Series Data Streams")

- **Hoeffding bound** (eq. 3.3): `ε = sqrt(R² ln(1/δ) / (2n))`, where n is the number of examples
  the leaf has seen since it was created. Each leaf k has its own ε_k.
- **Settling the diameter** (eq. 3.4): let d1 be the largest pairwise dissimilarity in leaf C_k
  (pair x1, y1) and d2 the second largest. "Let ∆d = d1 − d2 be a new random variable… if ∆d > ε_k,
  we can confidently say that, with probability 1 − δ, the difference between d1 and d2 is larger
  than zero", so diam(C_k) = d1. The split test runs only once this holds.
- **Split** (eq. 3.5): split C_k if `(d1 − d0)·|d1 + d0 − 2d̄| > ε_k`. Here d0 is the minimum and d̄
  the mean pairwise dissimilarity. The paper motivates this as a "high difference between
  (d1 − d̄) and (d̄ − d0)", related to the global d1 − d0. The absolute-value bars are lost in the
  PDF text extraction. River's code has them.
- **Split mechanics**: x1 and y1 become pivots, every other variable goes to the closest pivot, and
  "New leaves start new statistics": nothing is carried over from the parent.
- **Ties** (§3.4): a parameter τ. "At any time, if τ > ε_k the system applies the tests", taking the
  highest distance as the diameter. This caps how long a leaf waits when distances are nearly equal.
- **Aggregate on drift** (§3.3, eq. 3.6): for leaf C_k with sibling C_s and parent C_j, define
  `∆a = 2·diam(C_j) − (diam(C_k) + diam(C_s))`. Aggregate (collapse both children back into C_j,
  restart statistics) if `∆a < ε_j`, "supported by the confidence given by the parent's consumed
  data". The idea is that the children's diameters should not add up to as much as twice the
  parent's.
- **When to check**: the dissimilarity matrix is computed only when a leaf is tested, "after
  receiving a minimum number of examples" (§3.2.1). The models use first-order differences to reduce
  autocorrelation, which would otherwise violate Hoeffding's i.i.d. assumption.
- **Settings in the experiments** (§4): δ = 0.05, τ = 0.02.

### Earlier variants (same authors)

- *"Clustering of Time-Series Data Streams"*. The PDF does not name its venue. **Every n_min
  examples, test each leaf for aggregation; if a leaf was not aggregated, test it for splitting.**
  The range R = 1 is stated explicitly. Split if `max(d) − α·min(d) > ε`, with α = 1.4. Aggregate if
  `diam_k + diam_s > diam_j + ε_j`. The experiments use n_min = 1000.
- *"Hierarchical Time-Series Clustering for Data Streams"*. Clusters are tested in descending order
  of diameter, with at most one split or aggregation per n_min examples. The criteria use the
  divisive coefficient: aggregate if `DC_curr − DC_test < ε`.

### Rule as implemented in River (secondary; cites TKDE 2008)

`river/cluster/odac.py`, checked every `n_min` examples (default 100):

- split if `((d1 − d2) > e or tau > e) and (d1 − d0)·|d1 + d0 − 2·avg| > e`
- aggregate parent if `d1 − parent.d1 > max(parent.e, e)`, meaning the child's diameter exceeds the
  parent's
- `e = sqrt(ln(1/confidence_level) / (2n))` with R = 1

The aggregate rule matches River's docstring ("checks if one of the cluster's children has a
diameter bigger than their parent"). It differs from SDM eq. 3.6, so the TKDE rule is probably this
one. That is **[unverified]** against the paper.

### Guarantees claimed
- Hoeffding confidence 1 − δ that d1 really is the diameter. There is no guarantee for the tree as a
  whole.
- Constant memory and constant time per example with respect to the number of examples (§3.5). Each
  split on a leaf with n variables removes between n − 1 and (n/2)² pairwise dissimilarities.

### Cost per insertion
O(Σ_leaves |leaf|²) to update pair statistics. A test costs the same.

### Transfer to points
ODAC's statistics are means over examples of per-pair products. For point clustering, the
analogous i.i.d. stream is the sequence of **points arriving at a leaf**. Any per-point bounded
statistic (for example a per-point gain in squared error) can take Hoeffding's place. The idea
carries over in two stages: first "is the best choice settled?" (d1 − d2 > ε, or ε < τ), then "is it
worth splitting?" (heuristic > ε). Rules 6a–6b below follow this.

### Sources
- SDM 2006 PDF (author page): https://www.dcc.fc.up.pt/~jpp/publications/PDF/ODAC.pdf
- "Clustering of Time-Series Data Streams": https://www.dcc.fc.up.pt/~jpp/publications/PDF/kdds.pdf
- "Hierarchical Time-Series Clustering for Data Streams": https://www.dcc.fc.up.pt/~jpp/publications/PDF/htscds.pdf
- TKDE 2008 (not accessed): https://doi.org/10.1109/TKDE.2007.190727
- River: https://github.com/online-ml/river/blob/main/river/cluster/odac.py

---

## 2. SPFresh / LIRE (Xu et al., SOSP 2023)

### Rule
- **NPA** (§3.2): "each vector should be put into the nearest posting".
- **Split trigger** (§3.2, §4.2.1): a posting that exceeds a length limit is split. The Local
  Rebuilder first garbage-collects deleted vectors and splits only if the posting is still over the
  limit.
- **How a posting is split** (§4.2.1): "LIRE evenly splits the oversized posting into two smaller
  ones" using "the multi-constraint balanced clustering algorithm in [67]" (SPANN). In SPTAG this is
  `Split()` calling `KmeansClustering` with k = 2 (comment: "k = 2 … now it is fixed"). The
  assignment cost is `distance + λ·count[k]` (BKTree.h), a balance penalty. If clustering returns a
  single cluster, the posting is truncated instead.
- **Reassignment candidates after a split** (§3.3). A_o is the old centroid and A_1, A_2 are the new
  ones.
  - Eq. 1, a vector v **in the split posting**, is a candidate if `D(v, A_o) ≤ D(v, A_i) ∀ i ∈ {1,2}`.
    If either new centroid is closer than A_o, no neighbor B can beat it, because
    D(v,B) > D(v,A_o) held before the split.
  - Eq. 2, a vector v **in a nearby posting with centroid B**, is a candidate if
    `D(v, A_i) ≤ D(v, A_o) for some i`.
  - Both are *necessary* conditions. For each candidate, LIRE "searches v's new closest posting,
    then performs NPA check to get rid of false-positives". If a move is needed, it appends v to the
    new posting and deletes it from the old one.
- **Which neighbors**: "selecting several A_o's nearest postings". The parameter study (§5,
  Fig. 11) varies the range from 0 (split posting only) to 128 nearest postings and picks **64**
  ("marginal increase from range 64 to 128").
- **Code** (`CollectReAssign` / `CheckIsNeedReassign`, ExtraDynamicSearcher.h):
  - Split-posting vectors are reassigned when `D(v, A_o) < D(v, A_i)`, where A_i is the new head
    they landed in.
  - The neighbors are the `ReassignK` nearest heads to A_o. A neighbor vector is skipped if A_o is
    no farther than both new heads, or if its current head B is no farther than both new heads. So
    the code adds a second filter, `min_i D(v,A_i) < D(v,B)`.
  - `ReassignK` defaults to 0 in ParameterDefinitionList.h, so the neighbor pass is off unless
    configured.
- **Merge** (§3.2): when a posting is "smaller than a lower threshold", it is merged with its
  nearest posting. The shorter one is deleted and its vectors are appended to the other. Only the
  deleted posting's vectors need reassigning, since deleting a centroid "does not break NPA
  compliance of vectors from undeleted postings". Merges are triggered by the searcher. The code
  has `MergeThreshold` = 10 and merges only if `currentLength + nextLength < m_postingSizeLimit`.

### Guarantees claimed
- §3.4: split–reassign cascades **terminate**. Each split increases |C| by exactly one and |C| ≤ |V|,
  so the cascade length is at most |V| − |C|. There is no quality guarantee beyond NPA after
  reassignment.

### Cost per insertion (claims)
- Abstract: "only 1% of DRAM and less than 10% cores needed at the peak compared to the
  state-of-the-art".
- §5: "only 0.4% insertion will cause rebalancing". The average number of splits is 2, the maximum
  is 160, and the cascade length is 3. Merges occur at 0.1% of updates. "each time 5094 vectors are
  evaluated, and only 79 are actually reassigned."

### Sources
- Paper: https://arxiv.org/pdf/2410.14452 (also https://www.microsoft.com/en-us/research/uploads/prod/2023/08/SPFresh_SOSP.pdf)
- Code: https://github.com/microsoft/SPTAG/blob/main/AnnService/inc/Core/SPANN/ExtraDynamicSearcher.h,
  https://github.com/microsoft/SPTAG/blob/main/AnnService/inc/Core/SPANN/ParameterDefinitionList.h,
  https://github.com/microsoft/SPTAG/blob/main/AnnService/inc/Core/Common/BKTree.h

---

## 3. PERCH (Kobren, Monath, Krishnamurthy, McCallum, KDD 2017)

### Rule
- **Insert** (§3.2, Alg. 1): find the nearest-neighbor leaf ℓ, then `Split(ℓ)`. This creates a new
  internal node with children ℓ and the new leaf ℓ′ in ℓ's old place. AddPt runs on every ancestor.
- **Masking** (Def. 3, eq. 1): node v with sibling v′ and aunt a is masked if there is an x ∈ lvs(v)
  with `max_{y∈lvs(v′)} ‖x − y‖ > min_{z∈lvs(a)} ‖x − z‖`.
- **Masking rotation** (§3.3): after inserting ℓ′, check whether ℓ is masked. If so, Rotate. The
  rotation "swaps the positions of ℓ′ and its aunt" (Fig. 1). The code `_rotate()` makes v and the
  aunt siblings under a new parent, and v's old sibling takes the other slot. Repeat up the tree and
  stop at the first node that is not masked.
- **Bounding-box check used in practice** (§4.3, eq. 4): rotate if `d⁻(v, v′) > d⁺(v, a)`.
  - d⁻ and d⁺ are lower and upper box distances (eqs. 2–3), maintained in O(d) per node.
  - When eq. 4 holds, v is provably masked, and the rotation unmasks it.
  - Code: `is_closer_to_aunt()` compares `min(max-dist box bounds to aunt) < parent.children_min_d`.
- **Balance rotation** (§4.1, Def. 4): bal(v) = min(|lvs(vl)|,|lvs(vr)|)/max(…). Rotate if the
  rotation raises bal(T) **and** eq. 1 holds for some point. The code's `can_rotate_for_balance()`
  requires both an improvement in local balance and a guard against creating masking. It runs from
  the new leaf to the root after the masking rotations.
- **Collapsed mode** (§4.2): with at most L leaves, collapse the internal node whose maximum
  distance between children is smallest. Its children must both be leaves.

### Guarantees claimed
- Thm. 1: under separability (every within-cluster distance is smaller than every between-cluster
  distance), the greedy insertion plus masking rotations gives dendrogram purity 1, for any arrival
  order.
- Fact 3: balance rotations preserve purity under separability.
- Fact 4: collapse preserves purity if L > K.
- These hold for exhaustive eq. 1. The bounding-box test is stricter, so it may miss some masking.

### Cost per insertion
- O(d) box updates per ancestor, plus A* or beam nearest-neighbor search and the rotations.
- The depth is not bounded: "in the worst case the algorithm may explore all leaves… O(N²d)" in
  total. Empirically insertion is roughly logarithmic, helped by balance rotations (Fig. 5).

### Sources
- Paper: https://arxiv.org/pdf/1704.01858
- Code: https://github.com/iesl/xcluster/blob/master/src/python/xcluster/models/PNode.py

---

## 4. GRINCH (Monath, Kobren, Krishnamurthy, Glass, McCallum, KDD 2019)

### Rule
- **Insert** (§3.4, Alg. 3): attach the point as a sibling of its nearest leaf l, rotate while the
  rotation condition holds, then try `graft` from each ancestor.
- **Rotate** (eq. 1): for a linkage f (a similarity), rotate if `f(v, s(v)) < f(v, aunt(v))`, which
  swaps the sibling and the aunt.
  - Alg. 3 prints the loop condition as `f(x_i, s(x_i)) < f(aunt(x_i), s(x_i))`.
  - The code (`Grinch.rotate`) walks up from the nearest neighbor while
    `e_score(new, curr) < curr.parent.score()`, where `parent.score()` is f between the parent's two
    children. In effect the new point is attached above every node whose children prefer each other
    to it.
- **Graft** (§3.2, eq. 2, Alg. 1): find l, the nearest leaf to v outside lvs(v). Merge v and l as
  siblings if `f(v, l) > max[f(v, s(v)), f(l, s(l))]`, meaning they prefer each other to their
  current siblings. Otherwise:
  - if l prefers its sibling, set l ← par(l)
  - if v prefers its sibling, set v ← par(v)
  - stop when one of them becomes an ancestor of the other.
- **Restruct** (§3.3, Alg. 2): after a graft, from z, the previous sibling of the grafted node, up
  to r = lca:
  - let m be the most similar node among the siblings of z's ancestors
  - if `f(z, s(z)) < f(z, m)`, swap s(z) and m.
- **Approximations** (§4.1):
  - caps: no rotations, grafts or restructures above height h (h = 100 in the experiments)
  - single elimination: stop grafting after both nodes reject
  - one k-NN search per insertion, with k ∈ [25, 50]
  - approximate NN search with a navigable small world graph.
- **Linkages used**: approximate average linkage, and cosine similarity of the sum (centroid)
  vectors, called cosine linkage (§4.3).

### Guarantees claimed
Thm. 1 assumes f satisfies model-based separation (Def. 1): any two vertex sets whose union is
connected in G score higher than a pair whose union is not. Under it, GRINCH produces a tree in
which the ground truth is a tree-consistent partition, for any arrival order. Rotations alone
recover only clique-structured clusters (§3.1). Grafts are what recover chain-structured ones.

### Cost per insertion
The pages read give no bound; the paper's claim is empirical: "orders of magnitude faster" than HAC,
which costs O(n² log n). Ablation (Table 1): removing grafts lowers dendrogram purity by 3% (ALOI)
and 12% (synthetic). Removing rotations as well costs a further 6% and 2%.

### Sources
- Paper: https://arxiv.org/pdf/2001.00076
- Code: https://github.com/iesl/grinch/blob/master/src/scala/grinch/core/Grinch.scala and
  `GrinchNode.scala` in the same directory.

---

## 5. Contrast: BIRCH and ClusTree

**BIRCH** (Zhang, Ramakrishnan, Livny, SIGMOD 1996, §4.1–4.2). The copy I read is a scan with a
noisy OCR layer, so these are close paraphrases.
- A CF-tree has a branching factor B (at most B entries per nonleaf) and at most L entries per leaf.
  B and L follow from the page size P. Each leaf entry must satisfy "the diameter (or radius) has to
  be less than T".
- **Insert**: descend to the closest child, find the closest leaf entry L_i, and absorb if the
  threshold still holds. Otherwise add a new entry. If the leaf is then full, split it: "Node
  splitting is done by choosing the farthest pair of entries as seeds, and redistributing the
  remaining entries based on the closest criteria". Splits propagate upward like a B+-tree.
- **Merging refinement**: at the node N_J where split propagation stops, find the two closest
  entries. If they are not the pair produced by the split, merge them and their children, and
  re-split if the result overflows.
- BIRCH notes that splits are "caused by the page size, which is independent of the clustering
  properties of the data". It leaves the fix to later global phases 3 and 4.
- Source: https://www.math.unipd.it/~dulli/corso04/zhang99birch.pdf

**ClusTree** (Kranen, Assent, Baldauf, Seidl, KAIS 29:249–272, 2011, doi:10.1007/s10115-010-0342-8).
- I could not access the paper. Only its abstract (https://moa.cms.waikato.ac.nz/the-clustree-indexing-micro-clusters-for-anytime-stream-mining/)
  was available: an index of micro-clusters, anytime insertion, exponential aging, aggregation for
  fast streams.
- The MOA implementation from the authors' group (RWTH):
  - nodes have 3 entries
  - an insertion first fills a free slot, else overwrites an entry whose weight has decayed below a
    threshold
  - otherwise it splits, if the tree may grow or an outdated entry exists on the path
  - otherwise it merges the two closest entries
  - a split pairs the 4 entries into the (2,2) partition that minimizes the sum of within-pair
    distances.
- The paper's own split rule is **[unverified]**.
- Code: https://github.com/Waikato/moa/blob/master/moa/src/main/java/moa/clusterers/clustree/ClusTree.java

Neither method uses statistical evidence to split. Both split on capacity.

---

## 6. Implementing in our tree

Our setting is a k-ary tree (2–8 children per node). Every node has a running-mean center. Points
descend greedily by squared Euclidean distance on unit vectors, so `‖x−y‖² = 2 − 2cos ∈ [0,4]`.
Leaves hold point ids. A split picks k by BIC (`bic()` in `tree.rs`). Everything in this section is
**[ours]** unless it cites one of the sections above.

### 6a. Confidence-based split (ODAC-style, two stages) [ours]

ODAC (§1) waits until (i) its choice is settled and (ii) the gain beats ε. We do the same with a
per-point statistic that is an honest mean of i.i.d. bounded terms:

1. **Cross-fit.** Every `n_min` new points (ODAC's check cadence), split the leaf's members at random
   into halves A and B. Fit k-means on A for each k ∈ 1..8. On B, compute for each point
   `g_k(x) = ‖x − μ_A‖² − min_j ‖x − c_j^A‖²`, where μ_A is A's mean. Because the centers come from A,
   the g_k(x) on B are i.i.d. given A, which is what Hoeffding needs. In-sample BIC is not: its
   per-point terms share the fitted centers.
2. **Is the best k settled?** (the analog of ODAC eq. 3.4). Take the two k with the best BIC
   computed on B's SSE, say k* and k₂. Accept k* if the mean of `g_{k*}(x) − g_{k₂}(x)` exceeds ε,
   or if ε < τ (the analog of ODAC §3.4), where `ε = R·sqrt(ln(1/δ)/(2|B|))`.
3. **Is the split worth it?** (the analog of eq. 3.5). Split if the mean of g_{k*} minus ε exceeds
   the per-point gain at which BIC breaks even. From `bic()`,
   ΔBIC(1→k)/n = d·ln(s₁/s_k) − 2H(w) − price·(k−1)(d+1)·ln(n)/n, where s is the mean squared error
   and H(w) is the entropy of the child weights. Setting this to zero gives the break-even gain
   `ḡ* = s₁·(1 − exp(−[2H(w) + price·(k−1)(d+1)ln(n)/n]/d))`.
4. **Range.** g ∈ [−4, 4] in the worst case, so R = 8, which is very loose. With |B| = 300 and
   δ = 0.05, ε ≈ 0.57, while typical gains in 64 dimensions are around 0.1. Two options, both
   **[ours]**:
   - use the empirical range of g on B, a heuristic that loses the formal guarantee;
   - replace Hoeffding with a variance-aware bound or a bootstrap lower confidence bound on mean g.

   This note did not verify a primary source for the empirical-Bernstein bound or for
   stability-based selection of k, so they are named here only as directions.
5. **On failure**, keep accumulating rather than doubling `next_try` blindly. τ bounds the wait at
   n ≈ R² ln(1/δ)/(2τ²) points per half.

### 6b. Revisiting a split: held-out gain and ODAC-style aggregation [ours]

The points that reach an internal node P after it split are a held-out sample for that split. For
each such x, track `h(x) = ‖x − c_P‖² − ‖x − c_child(x)‖²` as a running mean and count since the
split (c_P is P's center).
- **Aggregate**, the analog of ODAC eq. 3.6 and River's child-exceeds-parent test: if
  `mean(h) + ε_P < ḡ*_P` (an upper confidence bound below the break-even gain), the split no longer
  pays. Collapse P's subtree back into one leaf holding all descendant ids. That leaf then
  re-splits under 6a with all its points, which is exactly the middle-level fix that a pass over
  the level gets.
- **Re-split check**, our analog of SPFresh's local rebuild: when P's descendant count has grown by
  a factor m (for example 4) since its split, re-run 6a on a sample of P's descendants. If the
  settled k* or the partition (ARI against the current children) differs, rebuild P's subtree from
  those points. Cost: one split's k-means on a sample of up to `SPLIT_SAMPLE`, done rarely.

### 6c. LIRE-style reassignment after a split [ours]

In SPFresh a split replaces a centroid at the same level, so neighbors can change their nearest
posting. In our tree, splitting leaf L adds children *under* L and leaves every center at or above
L's level unchanged. Members of L go to their nearest child, so LIRE Eq. 1 holds by construction and
Eq. 2 is vacuous for greedy descent. Our violations come from **drift**: centers are running means,
so earlier points may no longer descend to the leaf that holds them. The invariant to keep is
*descent consistency* (a point's leaf is where greedy descent puts it now), not flat NPA.
- **When**: after any split, aggregation or rebuild at node P. Also whenever a center has moved by
  more than η since it was last checked.
- **Which leaves**, the analog of LIRE's "nearest postings": the leaves under P's parent (P's
  siblings' subtrees), plus the leaves under the m siblings of P whose centers are nearest c_P.
  SPFresh found 64 neighbors enough at billion scale. In a tree with at most 8 children, the parent
  and grandparent neighborhoods are the natural local range.
- **Candidate filter**, the analog of LIRE eqs. 1–2: a point x in leaf S is a candidate if, at some
  ancestor a of S whose children's centers changed, the descent margin is small:
  `‖x − c_{child on x's path}‖² − min_{other child} ‖x − c_other‖² > 0`. Only the changed nodes need
  checking, as in LIRE. For speed, check only points whose stored margin at that level was below
  the center's displacement bound: `|Δ‖x−c‖²| ≤ 2‖Δc‖·‖x−c‖ + ‖Δc‖²`.
- **Reassign**, the analog of LIRE's NPA check: re-descend x from that ancestor. Move it only if it
  lands in a different leaf, then update both leaves' counts and centers. As in SPFresh's merge
  rule, only points of the changed nodes need checking.
- **Undersized leaves**, from LIRE's merge: if a leaf falls below `MIN_SHARE` of its parent, delete
  it and re-descend its points, which are the only ones affected.

### 6d. Rotate and graft for a centroid tree [ours]

PERCH and GRINCH are binary, point-leaf trees, and their guarantees need separability on points
(PERCH) or a linkage that separates the graph (GRINCH). Centroid linkage does not generally satisfy
either, so we would keep their *conditions* and drop their guarantees. GRINCH's cosine linkage is
itself a centroid linkage (§4), so this is the closest literature analog.
- **Reparent to an uncle**, the k-ary analog of GRINCH eq. 1 with f = −‖c_u − c_w‖²: for a child v of
  P, let U be the sibling of P whose center is nearest c_v. Move v under U if
  `‖c_v − c_U‖² < ‖c_v − c_P‖² − margin`. In that state the grandparent routes most of v's points to
  U, so v is effectively unreachable (a descent-consistency violation), which is exactly what
  PERCH's masking describes.
  - Recompute the centers from counts: `c_P ← (n_P c_P − n_v c_v)/(n_P − n_v)`, and the same for U.
  - Then run 6c on P and U.
  - Check on insertion along the descent path, with cost O(depth × branching × d), capped at low
    heights as in GRINCH §4.1.
  - The margin should be a confidence term in v's count, such as ε from 6a, so that noisy small
    nodes don't oscillate.
- **Graft** (GRINCH eq. 2): only worth it for chain-like clusters, which centroid descent cannot
  represent anyway. A cheap version: at a fixed depth, find nodes v and w with different parents
  that are each other's nearest center and closer than to their own parents' centers. Make them
  siblings under a new node. This changes the node count per level and interacts with `max_depth`,
  so it should come after 6a–6c.
- **Balance rotations** (PERCH): not needed. Our k-means splits with `MIN_SHARE` already bound how
  uneven the children can be.

---

## Could not verify from a primary source
- The ODAC TKDE 2008 equations: split condition, aggregation rule, τ and n_min handling. I used
  the SDM 2006 paper, two earlier author PDFs and River's code instead.
- The venue of "Clustering of Time-Series Data Streams" (kdds.pdf).
- The ClusTree paper's split and insertion rules; only the abstract and the MOA code were
  available.
- BIRCH: the text comes from a noisy OCR scan. The quoted sentences were legible, but the equation
  numbering was not checked.
- A per-insertion complexity bound for GRINCH.
- The empirical-Bernstein and clustering-stability literature mentioned in 6a.
