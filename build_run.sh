#!/bin/bash

set -eu

cargo build --profile test-release -p cli

cp target/test-release/cli zkevm-test-monitor/binaries/airbender-binary

cd zkevm-test-monitor/

./run test --arch airbender
