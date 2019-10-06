# Éole - Évaluateur Optimal de Lambda Expressions
A Lévy-optimal lambda evaluator without oracle

## Warning
Éole is *not proven correct*.
It may actually not work!
However, if it does not work for the full untyped lambda calculus, it may work for a fragment.

## Context
Éole is implemented following ideas from Levy, Lamping, Asperti, Lafont and many others.
Lambda expressions are converted into an interaction net (a computational model) which is then reduced.
A good introduction can be found in the book:
> Asperti, Andrea and Guerrini, Stefano. The optimal implementation of functional programming languages.
> Cambridge University Press, 1998.

### Summary
Interaction nets contain nodes that can interact (e.g. an application node can interact
with a lambda abstraction node). They work by applying local rewriting rules to *interacting* pairs of nodes.
They are a model of computation and can be use to implement a lambda calculus interpreter.
A lambda calculus interpreter is "Lévy-optimal" if it avoids duplicating work,
i.e. redexes, but also "virtual redexes" (things that might create redexes down the road).
This is done through "sharing" and "unsharing" which are represented respectively by "fan in" and "fan out" nodes.

When something is shared, a fan in and fan out delimiting the new share are created.
The tricky part is that several distinct shares may overlap.
When a fan in meets a fan out, is it the end of share (a "sharing" node meeting its "unsharing" counterpart),
or not (the nodes represent different shares)?
Answering that question is the main problem that Lamping was the first to solve with his algorithm.
> Lamping, John. An algorithm for optimal lambda calculus reduction.
> Proceeding POPL 1990, Pages 16-30.

The only problem is that the algorithm has a non negligible amount of overhead due to
a lot of bookkeeping operations. In some extreme cases, the bookkeeping is in O(2^n)
when the number of beta reductions is O(n).
> Julia Lawall and Harry G. Mairson. Optimality and Inefficiency : What Isn't a Cost Model of the Lambda Calculus? (1996)
> Proceedings ACM SIGPLAN 1996, Pages 92-101.

Beyond that, performing interactions in the nets amount to a pretty straightforward graph rewriting system
(special note: the garbage collection can also be challenging).
Hence, the "fans pairing" algorithm was given a special name: **the Oracle**.
In other words: do your interactions, and when two fans meet, ask the oracle what to do.
I like to think (probably inaccurately) about optimal reductions as:
> Optimal reduction = interaction net + oracle

### Éole approach
Éole changes are:
0. Its interaction nets are directed
0. It has two kinds of fans in:
    0. Stem fans, which are "sharer" and do not have a fan out counterpart
    0. Differentiated fans in, which with their fans out counterpart create "share boundaries"
0. A lazy labelling of differentiated fans in.

Stem fans differentiated themselves when crossing a lambda abstraction node.
A new unique label is created and assigned to the now differentiated fan in (going down the body of the abstraction)
and its matching fan out (following the binder).
In the current implementation, the lazy labelling is done through a global 64 bits counter.
This break the spirit of the *local rewriting rules*, but isn't a problem at all implementation wise.

More details, are to come in a paper (hopefully with a proof of Éole),
along with a discussion on the garbage collector and reduction strategies (the current implementation offers 2 of them, see below).

## Using the system (Linux instructions, probably works on mac too)

### Building
You need an up-to-date installation of Rust (but not a nightly release).
Just go in the main folder and:
```
cargo build --release
```
This will create an executable in the `target/release` folder.

Éole can generate `dot files` that can then be passed to [graphviz](https://graphviz.org/) to draw the several reduction steps.
Install graphviz if you want to visualize what is going on!
The `./dotgraph.sh` script creates the graphs and then assemble them using `pdfunite`
On Archlinux, pdfunite is provided by poppler.
Check your distribution for things around `poppler` (like `poppler-utils`) if you want it.


### First test
Launching
```
./target/release/eole
```
Should print the help and exit. Have a quick read!

The `tests` folder contains a several files you can try or use as example.
Let's start with something simple (linux style):
```
./target/release/eole tests/00_def.eole
```
which defines the identity function and the evaluates it (without an argument) should print:
```
(i5->i5)
```
In lambda calculus, we would write `λi5.i5`.
The `i` comes from the source and the `5` is an internal identifier.

### Generating graph
If you have `graphviz` installed, you can try
```
./launch.sh -G tests/04_delta.eole
```
Éole will create a `generated` folder and put a `dot files` per reduction step in it (plus the initial state of the network).

Note: The `launch.sh` script purpose is to clean any previous `generated` folder before calling `eole`,
forwarding all the arguments.
Then, it calls the `./dotgraph.sh` script, which generate the graphs.

The `-G` flag ask a graph per reduction step.
The `-g` flags only generate the initial and final graph.

*Never put anything in generated!* `./launch.sh` does a brutal cleaning...

* Black nodes are special nodes:
  * The root is used internally to anchor the graph.
  * Other black node are used by the garbage collector.
  * If it is a node "inside the graph", it is a temporary root for the reduction.
* The flashy green node represents the next interacting destructor.
* The flashy violet nodes represent the stack of nodes from the root to the next interacting node.
* Possible interaction are highlighted in red.

### Limiting the number of interaction
Some term can diverge, e.g. the term `(λi.i i)(λi.i i)` forever reduced into itself.
In that case, we can limit the number of interactions with the `-r` flag.
```
./launch.sh tests/05_delta_delta.eole -r 50
```
**Note:** this is a limit on the number of interactions, not β-reductions!

### Show me some stats
Add the `-v` flags:
```
./launch.sh tests/05_delta_delta.eole -r 50 -v
```
will indeed shows you that 50 is a limit on interaction: only 10 β-reductions were performed.

### Reduction strategies
By default, Éole uses a `full`strategy: it will reduce everything, i.e. if a term has a normal form, it will reach it.
For example, the following computes `3!` with Church numbers.
```
./launch.sh tests/benchmarks/fact03_noid.eole
```
This should print `(x38->(y52->(x38 (x38 (x38 (x38 (x38 (x38 y52))))))))`, i.e. 6 in Church number.
Note that the `full` strategy does not do useless work, i.e. it is *not* a "strict" or "call by value" strategy!

We can also use a lazy strategy with the '-s' flags:
```
/launch.sh tests/benchmarks/fact03_noid.eole -s lazy
```
And this will print something "bigger" because the lazy strategy stops at the weak head normal form.

### Limiting the read back
Some "small" graphs can represent quite "large" syntactic lambda terms.
The read back can be limited (in "depth" when travelling the graph) by the `-l` flag.
Try this:
```
./launch.sh tests/benchmarks/fact07_noid.eole -l 0 -v -g
```
Have a look at the graph, and then try this:
```
./launch.sh tests/benchmarks/fact07_noid.eole -v
```

### Memory options
The garbage collector can be deactivated with the `-m` flag.
Try this command and take a look at the memory used by the nodes:
```
./launch.sh tests/benchmarks/fact80.eole -v
```
Then compare with:
```
./launch.sh tests/benchmarks/fact80.eole -v -m none
```

By default, Éole never releases the memory.
This is can be seen by the `End allocation` stats,
showing the amount of memory used by the nodes just before terminating.
The compaction can be activated with the `-M` flags
```
./launch.sh tests/benchmarks/fact80.eole -v -M 1
```

## Syntax of Éole's file
A lambda abstraction is written with an arrow, e.g. `a->a` is `λa.a`
and application is done by juxtaposition.
A file can contains several definitions, terminated by a dot: `symbol = term.`
Finally, a file can contain one term to evaluate, also dot-terminated.

See the examples in the `tests` folder.



# TODO:

* Implement the "read" features (done in the parser but not in the interpreter).

* Fix design flaw in reduce full: right now, the fanstack compensate for it by storing an extra id.
  This extra id needs to be adjust when compacting, and can be costly (see ./tests/benchmarks/README.md).
