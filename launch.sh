#/bin/bash

DIR="generated"

rm -rf $DIR
mkdir $DIR
./target/release/eole $*
#./target/debug/eole $*
./dotgraph.sh
