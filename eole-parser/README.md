# Eole  Parser

The parser is generated with [LALRPOP](https://github.com/lalrpop/lalrpop)
through the script `build.rs`.

From the Rust documentation:

> The Rust file designated by the build command (relative to the package root) will be compiled and invoked before anything else is compiled in the package,
> allowing your Rust code to depend on the built or generated artifacts.
> By default Cargo looks for a "build.rs" file in a package root (even if you do not specify a value for build).
> Use build = "custom_build_name.rs" to specify a custom build name or
> build = false to disable automatic detection of the build script.

