#/bin/bash

DIR="generated"

rm -rf $DIR
mkdir $DIR
./target/release/eole $*
./dotgraph.sh
