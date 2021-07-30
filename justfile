# Defaults to test.
default:
    test

# Basic test. Use target "check" for a more in depth check up.
test:
    cargo test

# Check that all tests pass, and that formatting and coding conventions are OK.
check: test
    cargo clippy
    cargo fmt -- --check
    cargo doc
    cargo package --allow-dirty
    git status

# Run example based on its unique prefix (e.g. 00, 01, etc.).
run-example EXAMPLE:
    cargo run --example `basename examples/"{{EXAMPLE}}"* .rs`

# Run default application and all examples.
run-all:
    cargo run
    cargo run --example 00-transformations
    cargo run --example 01-geometric_geodesy
    cargo run --example 02-user_defined_macros
    cargo run --example 03-user_defined_operators

diff:  && status
    git diff

# Given check passes, commit what has been "git add"-ed
commit: check  &&  status
    git commit

# Given check passes, add everything and commit all changes
commit-all: check  &&  status
    git commit -a

# As commit-all but use MESSAGE as commit-message
commit-fast MESSAGE: check  &&  status
    git commit -a -m "{{MESSAGE}}"

# Git status
status:
    git status


# Some invisible oddities for general amusement

_sysinfo:
    @echo "This is an {{arch()}} machine, running {{os()}}".

_python:
    #!env python
    print('Hello from python!')
