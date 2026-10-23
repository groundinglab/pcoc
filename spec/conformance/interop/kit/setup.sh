#!/usr/bin/env bash
# user-local toolchain install for interop round 2. usage: setup.sh <ubuntu24.04|debian12>
set -u
OS=$1
T=~/interop-r2/toolchains; mkdir -p $T; cd $T
log(){ echo "[$(date +%T)] $*"; }
if [ ! -x zig/zig ]; then log zig; curl -fsSL https://ziglang.org/download/0.15.2/zig-x86_64-linux-0.15.2.tar.xz | tar xJ && mv zig-x86_64-linux-0.15.2 zig; fi
if [ ! -x jdk21/bin/java ]; then log jdk21; mkdir -p jdk21 && curl -fsSL https://api.adoptium.net/v3/binary/latest/21/ga/linux/x64/jdk/hotspot/normal/eclipse | tar xz -C jdk21 --strip-components=1; fi
if [ ! -x node22/bin/node ]; then log node22; F=$(curl -fsSL https://nodejs.org/dist/latest-v22.x/SHASUMS256.txt | awk '/linux-x64.tar.xz/{print $2}'); mkdir -p node22 && curl -fsSL https://nodejs.org/dist/latest-v22.x/$F | tar xJ -C node22 --strip-components=1; fi
if [ ! -x swift/usr/bin/swift ]; then log swift
  if [ $OS = debian12 ]; then U=https://download.swift.org/swift-6.1.2-release/debian12/swift-6.1.2-RELEASE/swift-6.1.2-RELEASE-debian12.tar.gz
  else U=https://download.swift.org/swift-6.1.2-release/ubuntu2404/swift-6.1.2-RELEASE/swift-6.1.2-RELEASE-ubuntu24.04.tar.gz; fi
  mkdir -p swift && curl -fsSL $U | tar xz -C swift --strip-components=1; fi
if [ ! -f /usr/include/openssl/sha.h ] && [ ! -f ssl/usr/include/openssl/sha.h ]; then log ssl-headers; mkdir -p ssl && (cd ssl && apt-get download libssl-dev && dpkg -x libssl-dev_*.deb .); fi
log done
ls $T; zig/zig version; jdk21/bin/java -version 2>&1|head -1; node22/bin/node -v; swift/usr/bin/swift --version 2>&1|head -1
