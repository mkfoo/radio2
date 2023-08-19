#!/bin/bash

set -e

build_rpm() {
    pushd "$1" > /dev/null
    rpmbuild -bb --build-in-place --target aarch64 "$2.spec"
    popd > /dev/null
}

build_rpm dqtt dqtt
build_rpm ulcd ulcd
build_rpm switch switchmon
build_rpm player hls_client
build_rpm player meta_client
