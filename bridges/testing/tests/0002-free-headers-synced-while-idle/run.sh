#!/bin/bash

set -e

source "${BASH_SOURCE%/*}/../../framework/utils/common.sh"
source "${BASH_SOURCE%/*}/../../framework/utils/zombienet.sh"

export ENV_PATH=`realpath ${BASH_SOURCE%/*}/../../environments/rococo-westend`

$ENV_PATH/spawn.sh &
env_pid=$!

ensure_process_file $env_pid $TEST_DIR/rococo.env 600
rococo_dir=`cat $TEST_DIR/rococo.env`
echo

ensure_process_file $env_pid $TEST_DIR/westend.env 300
westend_dir=`cat $TEST_DIR/westend.env`
echo

# Sleep for some time before starting the relayer. We want to sleep for at least 1 session,
# which is expected to be 60 seconds for the test environment.
echo -e "Sleeping 90s before starting relayer ...\n"
sleep 90
<<<<<<< HEAD:bridges/testing/tests/0002-mandatory-headers-synced-while-idle/run.sh
${BASH_SOURCE%/*}/../../environments/rococo-westend/start_relayer.sh $rococo_dir $westend_dir relayer_pid
=======
${BASH_SOURCE%/*}/../../environments/rococo-westend/start_relayer.sh $rococo_dir $westend_dir finality_relayer_pid parachains_relayer_pid messages_relayer_pid
>>>>>>> polkadot-v1.12.0:bridges/testing/tests/0002-free-headers-synced-while-idle/run.sh

run_zndsl ${BASH_SOURCE%/*}/rococo-to-westend.zndsl $westend_dir
run_zndsl ${BASH_SOURCE%/*}/westend-to-rococo.zndsl $rococo_dir

