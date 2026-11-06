# Bounded immutable line diffs

`UnifiedDiff(left,right[,context]).string()` and
`ContextDiff(left,right[,context]).string()` snapshot Sequencing<string> inputs
once and cache their representation. Context defaults to three and must be
0..64. Equal inputs print an empty string; changed inputs use fixed `left`/`right`
labels, correct empty/single/multiple-line ranges, and merged context hunks.
Context replacements use `!` on both sides. No files are read.

`MatchingBlocks(left,right).values()` returns ordered MatchingBlock values with
left()/right()/size(), including the terminal `(leftSize,rightSize,0)` sentinel.
`MatchingRatio(left,right).value()` is `2*LCS/(leftSize+rightSize)`, or one for two
empty inputs. This is an explicit LCS ratio, not an unspecified Similarity metric.

Each input has at most 512 lines, 4,096 Unicode scalars per line and 1,048,576
scalars total. Embedded LF is rejected; callers choose line splitting explicitly.
Output is at most 1,048,576 scalars. Invalid inputs raise InvalidDiff; oversized
rendering raises DiffLimitExceeded. Native backend failures map to UnavailableDiff.

`runtime/diff::diff_script(@string,@string)` uses Algorithm::Diff 1.201 LCSidx.
It returns only bounded integer edit/left-position/right-position triplets; domain
models, matching blocks and both renderers remain std code. No arbitrary object
crosses that boundary. Equal-cost ties are backend-deterministic; the contract
promises a shortest insertion/deletion script, not a Python difflib tie policy.
The portable provider repeats validation and returns NativeFailure on errors.

The original pure Myers prototype passed a separate dynamic-programming and patch
oracle but took 59.26 seconds for 64 completely changed lines. The bounded native
matcher reduces the same case to 0.778 seconds including std rendering. Evidence
and remaining costs are in the performance investigation.
An independent exhaustive oracle verifies 225 short sequence pairs, both rendered
patches, shortest edit counts, blocks, and the terminal sentinel.

```okkama
import "diff" take UnifiedDiff,MatchingRatio;
let before is @string=["hello","old"];
let after is @string=["hello","new"];
let change=UnifiedDiff(before,after,1).string();
let ratio=MatchingRatio(before,after).value();
```
