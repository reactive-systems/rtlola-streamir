The `streamir-lib` crate contains common functionality for all tools working with the StreamIR.
It contains the `StreamIR` representation, as well as the lowering from the output of the Frontend into the StreamIR.
It defines the rewriting rules and provides a mechanism for applying the rules until a fixed point is reached.
Furthermore, it provides a framework for easily translating the StreamIR into a target language.

In the following, we will explain parts of the framework which are useful for implementing new rewriting rules or new target languages.

## Rewriting Rules
The rewriting rules are defined in the module `rewrite_rules`.
Each rule implements the `RewriteRule` trait, which defines how parts of the StreamIR are rewritten.
It provides the methods `rewrite_stmt` and `rewrite_guard`, which are applied to each statement/guard recursively.
Likewise, the methods `rewrite_memory` and `rewrite_buffer` are to define memory optimizations.
All methods are implemented as a no-op by default, and only the required methods need to be overwritten.

## Binary
The `streamir-lib` provides a binary for displaying and debugging the resulting StreamIR when implementing new rewriting rules.
The StreamIR is represented with parallel statements stacked horizontally, while sequential statements are stacked vertically.
Simply run the binary with the specification file:
```
$ streamir-lib test.lola
if @a then
    shift a
    -------
    input a
-----------------------------------------------------
if @a then  | if @a then  | if @a then
    shift b |     shift c |     shift d
-----------------------------------------------------
if @a then                | if @a then
    eval_0 b with (a()+1) |     eval_0 c with (a()-1)
-----------------------------------------------------
if @a then
    eval_0 d with (b()+c())
```
and add the `--optimize-all` argument for displaying the StreamIR after applying all rewriting rules:
```
$ streamir-lib test.lola --optimize-all
if @a then
    input a
    ---------------------------------------------
    eval_0 b with (a()+1) | eval_0 c with (a()-1)
    ---------------------------------------------
    eval_0 d with (b()+c())
```

## Formatter
For the easy generation of target language code based on the StreamIR, we provide a framework for formatting.
The user simply has to implement a series of traits defining how the parts of the StreamIR are represented in the target language.
The main trait required is the `StreamIRFormatter`, and further there are traits `StmtFormatter`, `ExprFormatter`, `GuardFormatter` and `TypeFormatter`, which can be used if required. For the generation of code (in contrast to for example the generation of closures with the JIT compilation), we provide the `DefaultStmtFormatter` etc. traits which contain sensible default implementations for a lot of target programming languages.

## Files Formatter
A lot of programming languages are separated into different files.
For this purpose, the library contains a `FilesFormatter` trait, which allows to easily generate a set of output files.
It also introduces the concept of `Requirements`, which can be added to the files.
A requirement is a block of code which is required by another.
For example, to call a function, the function definition is added as a requirement to the corresponding file and the call to the function is returned by the formatter as, for example, part of an expression.