The compilation of the StreamIR to Rust is provided as a binary `rtlola2solidity`.
It takes as input the path to an rtlola specification and a specification of the function interface in TOML.
Each function is represented by a `[[function]]` block with an associated name, and contains a set of `[[function.argument]]`'s, which represent the arguments to this function and provides inputs to the associated input streams.

For usage of the compiler, consider the documentation of the command line interface using `rtlola2solidity --help`.