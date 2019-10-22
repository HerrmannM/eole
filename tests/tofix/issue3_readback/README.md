# Readback,  issue #3

We compare the output of eole with the one from https://crypto.stanford.edu/~blynn/lambda/.
Using vim, you can easily convert a eole term into the syntax of the above site with the regexp/replace:
```
:%s/\([a-z0-9]\+\)->/\\\1 ->/g
```

The test are done here without a GC, and with a modify version of the net that do not reuse old nodes.

The file `./issue_003_01.eole` contains one of the problematic terms in the issue #3.
The result contains a subterm `(m48->m48)` where the above website shows `(λm1.m)`.

The file `./min_pb.eole` derived from the above show a similar problem.
```

```
