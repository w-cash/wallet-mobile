#!/usr/bin/env bash
set -Eeuo pipefail

test_name="ZingoTests"
set_test_name=false
run_live=false
destination="${WCASH_IOS_DESTINATION:-platform=iOS Simulator,name=iPhone 17,OS=latest}"
funded_seed="${WCASH_IOS_FUNDED_TEST_SEED:-}"
unset WCASH_IOS_FUNDED_TEST_SEED

usage() {
    printf '%s\n' \
        "Run the Wcash iOS XCTest suite. Full Xcode and an iOS simulator are required." \
        "" \
        "  -e TEST   Select a test target, class, or method (default: ZingoTests)" \
        "  -l        Run the Wcash Local Regtest class against 127.0.0.1:48234" \
        "  -h        Show this help"
}

while getopts ':e:lh' option; do
    case "$option" in
        e)
            test_name="$OPTARG"
            set_test_name=true
            ;;
        l)
            run_live=true
            ;;
        h)
            usage
            exit 0
            ;;
        :|?)
            usage >&2
            exit 2
            ;;
    esac
done

if [[ ! -d ios ]]; then
    printf 'Run %s from the wallet-mobile repository root.\n' "$0" >&2
    exit 2
fi

if ! xcodebuild -version >/dev/null 2>&1; then
    printf 'Full Xcode is required. Select it with sudo xcode-select -s /Applications/Xcode.app.\n' >&2
    exit 2
fi

if [[ "$run_live" == true ]]; then
    if [[ "$set_test_name" == false ]]; then
        if [[ -n "$funded_seed" ]]; then
            test_name="ZingoTests/WcashLocalRegtestTests"
        else
            test_name="ZingoTests/WcashLocalRegtestTests/testCreateRestoreSyncReceiveAndHistory"
        fi
    fi
    if ! nc -z -w 2 127.0.0.1 48234; then
        printf 'Wcash CompactTxStreamer is unavailable on 127.0.0.1:48234.\n' >&2
        exit 2
    fi
fi

export RCT_NO_LAUNCH_PACKAGER=1

if [[ -d ios/build/DerivedData/Build/Products ]]; then
    find ios/build/DerivedData/Build/Products \
        -maxdepth 1 -type f -name 'Zingo_*.xctestrun' -delete
fi

if ! xcodebuild build-for-testing \
    -workspace ios/Zingo.xcworkspace \
    -scheme Zingo \
    -sdk iphonesimulator \
    -configuration Debug \
    -destination "$destination" \
    -parallel-testing-enabled NO \
    -derivedDataPath ios/build/DerivedData \
    ARCHS=arm64 \
    ONLY_ACTIVE_ARCH=YES \
    DEVELOPMENT_TEAM='' \
    COMPILER_INDEX_STORE_ENABLE=NO; then
    printf 'Wcash iOS integration test build failed.\n' >&2
    exit 1
fi

test_arguments=()
if [[ "$set_test_name" == true || "$run_live" == true ]]; then
    test_arguments+=(-e "$test_name")
fi
if [[ "$run_live" == true ]]; then
    test_arguments+=(-l)
fi

if [[ -n "$funded_seed" ]]; then
    export WCASH_IOS_FUNDED_TEST_SEED="$funded_seed"
fi
scripts/ci/ios_integration_tests_ci.sh "${test_arguments[@]}"
