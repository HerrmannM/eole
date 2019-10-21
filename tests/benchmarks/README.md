# Benchmarking

* Factorial and fibonacci examples are taken from:
>  Asperti & Chroboczek,
>  Safe Operators: Brackets Closed Forever
Those examples are primitive recursive, hence do not use a paradoxal combinator.
They also apply the identity to the result, in order to trigger a full reduction (into the identity).
This is mandatory for lazy systems, but not for our default strategy (try with `primfact_3_noid.eole`)
which will happily take more than a lifetime to output 80!, so be ready to CTRL-C if you experiment a bit (yes, "to CTRL-C" is a verb).
  * Due to different ways to express the examples, we have a constant delta in the number of family reductions (+23).

* The `power_mod` is taken [a stack overflow question](from https://stackoverflow.com/questions/31707614/why-are-%CE%BB-calculus-optimal-evaluators-able-to-compute-big-modular-exponentiation).
  * Try with (-M 1) and without (default) the compactor. Yeah, the compactor cost a lot (in full-default mode)

* To check: definition of the "operators" 'Add' and 'Mult' used to influence greatly the performance of a previous prototype.
