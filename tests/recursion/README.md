# Recursion test

Testing different from of recursion with 3!.
Works best with the `full` reduction strategy (default).
Launch the example with `-g` (to output the first and last graph):
we can see that the garbage is not fully collected, and this is because we do not reduce useless redexes.
However, we need to improve:
  * A "global" analysis checking if a part of the graph is disconnected from the root (disregarding the erase node).
    * Probably too costly, not local... does not seam elegant to me.
  * Actually reduce the garbage and introduce more local interactions,
    e.g. fan with an erased aux port could cross some other node to reach their paired fan "faster"
    (maybe it helps, maybe not...)
    * Their are some things about this in Asperti & Guerrini. To check.
    * Could be done in another thread
